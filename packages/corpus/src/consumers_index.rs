//! Who reads a law: the reverse of the references a law body makes.
//!
//! [`crate::source_map::collect_law_references`] answers which other laws a
//! body points at, at law level. Enriching one article needs the other
//! direction and a finer grain: which articles elsewhere read *this* output, so
//! that renaming or retyping it is visible as a break before it lands. No such
//! index existed; everything that needed it (the integrity script, the corpus
//! metrics) rebuilt the forward half and stopped there.
//!
//! The index is built from per-law [`SourceBinding`] lists, so a caller that
//! already walks the corpus (a tar pass over a GitHub snapshot, a disk scan of
//! a checkout) collects the bindings in the same pass and builds the index
//! without reading anything twice. Pure: no I/O, no clock.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// How an article reaches into another law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    /// `source: {regulation: …, output: …}`: reads one output.
    SourceOutput,
    /// `source: {article: "<law>#<article>"}`: calls the decision of an
    /// article. A `$`-target is resolved at run time and is not recorded.
    SourceArticle,
    /// `implements: [{law: …, open_term: …}]`: fills an open term of the
    /// higher law.
    Implements,
}

/// One reference from an article of this law to another law.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceBinding {
    /// Number of the article (entry) that carries the reference.
    pub article: String,
    /// `$id` of the law it points at.
    pub regulation: String,
    /// What it reads: the output name (`SourceOutput`), the target article
    /// (`SourceArticle`) or the open term (`Implements`). Empty when the body
    /// did not say.
    pub target: String,
    pub kind: BindingKind,
}

/// Collect every cross-law binding in a law body, per article.
///
/// A structural walk per article, for the same reason
/// `collect_law_references` walks: a binding at a nesting the schema grows
/// later must not silently drop out of a report whose job is to find them.
/// A body that does not parse yields nothing; bindings without a regulation
/// (intra-law `source: {output: …}`) are not cross-law and are skipped.
#[must_use]
pub fn collect_source_bindings(yaml: &str) -> Vec<SourceBinding> {
    let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml) else {
        return Vec::new();
    };
    let own_id = doc.get("$id").and_then(|v| v.as_str()).unwrap_or_default();
    let mut out = BTreeSet::new();
    let Some(articles) = doc.get("articles").and_then(|v| v.as_sequence()) else {
        return Vec::new();
    };
    for article in articles {
        let Some(number) = article_number(article) else {
            continue;
        };
        walk(article, &number, own_id, &mut out);
    }
    out.into_iter().collect()
}

/// `number:` as a string, whether the YAML quoted it or not.
fn article_number(article: &serde_yaml_ng::Value) -> Option<String> {
    match article.get("number")? {
        serde_yaml_ng::Value::String(s) => Some(s.clone()),
        serde_yaml_ng::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn walk(
    value: &serde_yaml_ng::Value,
    article: &str,
    own_id: &str,
    out: &mut BTreeSet<SourceBinding>,
) {
    match value {
        serde_yaml_ng::Value::Mapping(map) => {
            for (key, child) in map {
                match key.as_str() {
                    Some("source") => {
                        if let Some(regulation) = child.get("regulation").and_then(|v| v.as_str())
                            && !regulation.is_empty()
                            && regulation != own_id
                        {
                            let output = child
                                .get("output")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            out.insert(SourceBinding {
                                article: article.to_string(),
                                regulation: regulation.to_string(),
                                target: output.to_string(),
                                kind: BindingKind::SourceOutput,
                            });
                        }
                        if let Some((regulation, target)) = child
                            .get("article")
                            .and_then(|v| v.as_str())
                            .filter(|t| !t.starts_with('$'))
                            .and_then(|t| t.split_once('#'))
                            && !regulation.is_empty()
                            && regulation != own_id
                        {
                            out.insert(SourceBinding {
                                article: article.to_string(),
                                regulation: regulation.to_string(),
                                target: target.to_string(),
                                kind: BindingKind::SourceArticle,
                            });
                        }
                    }
                    Some("implements") => {
                        for item in child.as_sequence().into_iter().flatten() {
                            if let Some(law) = item.get("law").and_then(|v| v.as_str())
                                && !law.is_empty()
                            {
                                let open_term = item
                                    .get("open_term")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                out.insert(SourceBinding {
                                    article: article.to_string(),
                                    regulation: law.to_string(),
                                    target: open_term.to_string(),
                                    kind: BindingKind::Implements,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            for (_, child) in map {
                walk(child, article, own_id, out);
            }
        }
        serde_yaml_ng::Value::Sequence(items) => {
            for item in items {
                walk(item, article, own_id, out);
            }
        }
        _ => {}
    }
}

/// One article elsewhere that reads a law.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Consumer {
    /// `$id` of the reading law.
    pub law_id: String,
    /// The article in that law that carries the binding.
    pub article: String,
    /// What it reads (output name, article number or open term).
    pub target: String,
    pub kind: BindingKind,
}

/// Reverse index: for every law, the articles elsewhere that bind to it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumersIndex {
    by_law: BTreeMap<String, BTreeSet<Consumer>>,
}

impl ConsumersIndex {
    /// Build from `(law_id, bindings)` pairs, as one corpus walk yields them.
    pub fn from_bindings<I, B>(laws: I) -> Self
    where
        I: IntoIterator<Item = (String, B)>,
        B: IntoIterator<Item = SourceBinding>,
    {
        let mut by_law: BTreeMap<String, BTreeSet<Consumer>> = BTreeMap::new();
        for (law_id, bindings) in laws {
            for binding in bindings {
                if binding.regulation == law_id {
                    continue;
                }
                by_law
                    .entry(binding.regulation)
                    .or_default()
                    .insert(Consumer {
                        law_id: law_id.clone(),
                        article: binding.article,
                        target: binding.target,
                        kind: binding.kind,
                    });
            }
        }
        Self { by_law }
    }

    /// Every consumer of `law_id`, ordered by reading law and article.
    #[must_use]
    pub fn consumers_of(&self, law_id: &str) -> Vec<Consumer> {
        self.by_law
            .get(law_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Consumers of `law_id` grouped by what they read.
    #[must_use]
    pub fn consumers_by_target(&self, law_id: &str) -> BTreeMap<String, Vec<Consumer>> {
        let mut grouped: BTreeMap<String, Vec<Consumer>> = BTreeMap::new();
        for consumer in self.consumers_of(law_id) {
            grouped
                .entry(consumer.target.clone())
                .or_default()
                .push(consumer);
        }
        grouped
    }

    /// Number of laws that have at least one consumer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_law.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_law.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AWIR: &str = r#"
$id: awir
articles:
  - number: "8"
    machine_readable:
      execution:
        output:
          - name: toetsingsinkomen
"#;

    const ZORGTOESLAG: &str = r#"
$id: zorgtoeslag
articles:
  - number: 2
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: awir
              output: toetsingsinkomen
          - name: eigen
            source:
              output: iets_van_mezelf
          - name: besluit
            source:
              article: "awir#16"
          - name: dynamisch
            source:
              article: "$wet#3"
"#;

    const REGELING: &str = r#"
$id: regeling_standaardpremie
articles:
  - number: "1"
    machine_readable:
      implements:
        - law: zorgtoeslag
          open_term: standaardpremie
"#;

    #[test]
    fn collects_cross_law_bindings_per_article() {
        let bindings = collect_source_bindings(ZORGTOESLAG);
        assert_eq!(
            bindings,
            vec![
                SourceBinding {
                    article: "2".into(),
                    regulation: "awir".into(),
                    target: "16".into(),
                    kind: BindingKind::SourceArticle,
                },
                SourceBinding {
                    article: "2".into(),
                    regulation: "awir".into(),
                    target: "toetsingsinkomen".into(),
                    kind: BindingKind::SourceOutput,
                },
            ],
            "intra-law sources and run-time $-targets are not cross-law bindings"
        );
    }

    #[test]
    fn collects_implements_with_open_term() {
        let bindings = collect_source_bindings(REGELING);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].kind, BindingKind::Implements);
        assert_eq!(bindings[0].target, "standaardpremie");
    }

    #[test]
    fn unparseable_body_yields_nothing() {
        assert!(collect_source_bindings("articles: [").is_empty());
    }

    #[test]
    fn reverses_bindings_into_consumers() {
        let index = ConsumersIndex::from_bindings([
            ("awir".to_string(), collect_source_bindings(AWIR)),
            (
                "zorgtoeslag".to_string(),
                collect_source_bindings(ZORGTOESLAG),
            ),
            (
                "regeling_standaardpremie".to_string(),
                collect_source_bindings(REGELING),
            ),
        ]);
        let awir = index.consumers_by_target("awir");
        assert_eq!(awir["toetsingsinkomen"][0].law_id, "zorgtoeslag");
        assert_eq!(awir["toetsingsinkomen"][0].article, "2");
        assert_eq!(awir["16"][0].kind, BindingKind::SourceArticle);
        let zt = index.consumers_of("zorgtoeslag");
        assert_eq!(zt.len(), 1);
        assert_eq!(zt[0].law_id, "regeling_standaardpremie");
        assert!(index.consumers_of("onbekend").is_empty());
    }
}
