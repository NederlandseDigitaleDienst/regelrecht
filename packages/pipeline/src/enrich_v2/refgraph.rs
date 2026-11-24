//! One deterministic scan of what a law does to itself, and of where it
//! reaches outside.
//!
//! Two questions turned out to be the same question. *In welke volgorde moeten
//! de vensters?* and *welke bindingen kon een venster niet leggen?* both need
//! the same two indexes over one file: which entry references which other
//! entry, and which entry produces which value. Building them twice would let
//! them drift, so they are built once here and used by both callers —
//! `crate::enrich::plan_chunk` for the window boundary and
//! [`super::reconcile`] for the closing pass.
//!
//! No model, no shell, no network: everything below is derivable from the file.
//!
//! ## A node is `(wet, artikel)`
//!
//! The law is part of the key, not an assumption around it. A `references`
//! block carries its BWB number, so a reference across a statute boundary
//! reads exactly like one inside it, and the graph records both. Only the
//! intra-law half is used today: ordering across statutes needs an index over
//! the whole corpus and runs into the closure question RFC-026 owns (which
//! laws take part, where the closure stops, what counts as a known gap). The
//! data structure is shaped so that adding it later is a matter of feeding in
//! more documents, not of reshaping the graph.
//!
//! ## What the measurement said
//!
//! Ronde 4 leverde twee volledig verrijkte wetten op (Awir, 329 entries; Wet op
//! de zorgtoeslag, 35 entries). Daarin staan 57 intra-wet bindingen die vooruit
//! wijzen: de consument staat vóór de producent in het document. Dat is precies
//! de klasse die een venster niet kan leggen.
//!
//! - **Een bouwplan uit de verwijzingsgraaf lost dat niet op.** Een
//!   topologische orde over de hoofdartikelen (cycli gecondenseerd, gelijkspel
//!   op documentvolgorde) bracht die 57 op **69**. Documentvolgorde is al een
//!   goede bouwvolgorde: de wetgever zet definities en algemene bepalingen
//!   vooraan, en de verwijzingsgraaf draait dat juist om ("in afwijking van
//!   artikel 8" laat een later artikel naar een eerder wijzen zonder dat er
//!   een waarde overgaat). Daarom is [`Graph::plan_order`] er wel, maar staat
//!   hij standaard uit.
//! - **De vensterrand lost het wél op.** 48 van de 57 zitten binnen één
//!   hoofdartikel: een aanhef die bindt aan zijn eigen leden of onderdelen.
//!   Geen enkele herordening van artikelen raakt die; een vensterrand die een
//!   hoofdartikel niet doorknipt haalt ze allemaal binnen één venster. Bij de
//!   venstermaat die vandaag draait (2 entries) daalt het aantal bindingen dat
//!   buiten zijn venster valt van **46 naar 8**.
//!
//! ## Cycli
//!
//! RFC-026 stelt vast dat de graaf tussen wetten niet acyclisch is en dat er
//! dus geen bouwvolgorde bestaat; het antwoord daar is een sluitings-poort in
//! plaats van een volgorde. Binnen één wet geldt hetzelfde in het klein: de
//! Awir heeft één sterk samenhangende component van negen hoofdartikelen (2,
//! 3, 7, 8, 9, 13, 14, 16, 38). [`Graph::layers`] condenseert die tot één
//! knoop, zodat een cyclus heel in één laag landt in plaats van willekeurig
//! doorgeknipt te worden. Binnen zo'n component bestaat geen goede volgorde en
//! blijft de afrondende pass van [`super::reconcile`] nodig.

use std::collections::{BTreeMap, BTreeSet};

use serde_yaml_ng::Value;

use super::context::referenced_articles;

/// The top-level article an entry belongs to.
///
/// A dot does not mean a lid. Measured over both corpora, every one of the 271
/// dotted numbers is a whole article number in its own right: the BRP's `2.62`
/// opens with the words "Artikel 2.62" and the Wlz's `2.1.1` has the url
/// `#Artikel2.1.1` with its leden lettered separately. Cutting on the first dot
/// collapsed the BRP's 141 articles onto 4 nodes and the Woo's 53 onto 9, so
/// the graph said those articles were one and the brief went out with the wrong
/// neighbours.
///
/// What is a suffix is a segment that does not start with a digit: `3c.1` is
/// article `3c`, and `2.1.e.1°` is article `2.1` with onderdeel `e`. So the cut
/// falls at the first segment that opens with something other than a digit, and
/// a purely numeric number stays whole.
#[must_use]
pub fn top_article(number: &str) -> &str {
    let mut end = number.len();
    let mut at = 0;
    for (index, segment) in number.split('.').enumerate() {
        if index > 0 && !segment.starts_with(|c: char| c.is_ascii_digit()) {
            end = at - 1; // drop the dot that introduced this segment
            break;
        }
        at += segment.len() + 1;
    }
    &number[..end]
}

/// The entry an aanhef and its leden share, for keeping a window whole.
///
/// A different question from [`top_article`], and the two are easy to confuse.
/// This one groups the entries the enricher writes, where a lid is a dotted
/// suffix of the entry it belongs to (`1`, `1.1`, `1.2`), so the cut falls at
/// the first dot. `top_article` reads article numbers as the corpus writes
/// them, where `2.62` is a whole article of the BRP and cutting it would merge
/// articles that have nothing to do with each other.
///
/// They shared one function until that difference bit: fixing the corpus side
/// silently changed how windows were cut.
#[must_use]
pub fn entry_group(number: &str) -> &str {
    match number.find('.') {
        Some(i) => &number[..i],
        None => number,
    }
}

/// One node of the reference graph: a top-level article of a named statute.
///
/// The statute is part of the identity because a reference block gives it. A
/// graph over one document has one `bwb_id` among its own nodes and any number
/// among its outward edges.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node {
    /// BWB identifier of the statute the article belongs to.
    pub bwb_id: String,
    /// Top-level article number within that statute.
    pub article: String,
}

impl Node {
    /// The article `article` of the statute `bwb_id`.
    pub fn new(bwb_id: impl Into<String>, article: impl Into<String>) -> Self {
        Self {
            bwb_id: bwb_id.into(),
            article: article.into(),
        }
    }
}

/// Where one value is produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Producer {
    /// Entry number of the article that declares it as an `output`.
    pub entry: String,
    /// Index of that entry in document order.
    pub index: usize,
    /// The `execution.parameters` names that entry takes, in declared order.
    pub parameters: Vec<String>,
}

/// One binding a model declares: where a value it reads comes from.
///
/// Kept as written, unresolved: `regulation` names a law by `$id` and `output`
/// a value that law produces, and only an index over several laws can turn
/// that into an article (through the target's [`Graph::producers`]). Inside
/// one law, `regulation` is `None` and the own producers resolve `output`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    /// `$id` of the law read, or `None` for this law itself.
    pub regulation: Option<String>,
    /// The output read, from `source.output`.
    pub output: Option<String>,
    /// The article called, from `source.article` (`<law>#<article>`), as its
    /// article half.
    pub article: Option<String>,
}

/// The reference graph plus the value producer index.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// BWB identifier of the statute this graph was scanned from.
    pub bwb_id: String,
    /// Entry numbers of that statute in document order.
    pub entries: Vec<String>,
    /// Its top-level articles in document order, deduplicated.
    pub articles: Vec<String>,
    /// Every edge, keyed by the node it starts at. An edge means "that one
    /// first". Targets in another statute are kept: they are what a
    /// corpus-wide order would walk, and dropping them here would make the
    /// generalisation a rewrite instead of an extension.
    pub depends_on: BTreeMap<Node, BTreeSet<Node>>,
    /// What each top-level article's model reads, keyed by that article: the
    /// `source` blocks of its `machine_readable`. Where `depends_on` is what
    /// the text cites, this is what the model computes with, and the two
    /// differ both ways: a citation that passes no value, a value read from a
    /// law the text names only by its title.
    pub binds_to: BTreeMap<String, BTreeSet<Binding>>,
    /// Every output name this law declares, and where. A name declared by more
    /// than one entry maps to all of them — ambiguity that the closing pass
    /// refuses to resolve on its own.
    pub producers: BTreeMap<String, Vec<Producer>>,
    /// References that name another statute without naming an article in it:
    /// `(dit artikel, dat BWB-nummer)`. They are edges with no target, so the
    /// closure cannot follow them and records them as known gaps instead. On
    /// the Zorgverzekeringswet at depth 3 there are some three thousand of
    /// them, so following them would mean harvesting the statute book.
    pub outward_law_only: BTreeSet<(String, String)>,
}

impl Graph {
    /// Scan one law document.
    #[must_use]
    pub fn scan(doc: &Value) -> Self {
        let bwb_id = doc
            .get("bwb_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let articles_seq = doc
            .get("articles")
            .and_then(Value::as_sequence)
            .cloned()
            .unwrap_or_default();

        let mut graph = Self {
            bwb_id: bwb_id.clone(),
            ..Self::default()
        };
        for (index, article) in articles_seq.iter().enumerate() {
            let Some(number) = entry_number(article) else {
                continue;
            };
            graph.entries.push(number.clone());
            let top = top_article(&number).to_string();
            if !graph.articles.contains(&top) {
                graph.articles.push(top.clone());
            }

            for (name, producer) in outputs_of(article, &number, index) {
                graph.producers.entry(name).or_default().push(producer);
            }
            let own_id = doc.get("$id").and_then(Value::as_str).unwrap_or_default();
            let mut bindings = BTreeSet::new();
            if let Some(mr) = article.get("machine_readable") {
                collect_bindings(mr, own_id, &mut bindings);
            }
            if !bindings.is_empty() {
                graph
                    .binds_to
                    .entry(top.clone())
                    .or_default()
                    .extend(bindings);
            }

            let from = Node::new(&bwb_id, &top);
            let mut targets: BTreeSet<Node> = BTreeSet::new();
            // Prose names articles of this same statute; `referenced_articles`
            // already drops "artikel 8 van de Zorgverzekeringswet".
            if let Some(text) = article.get("text").and_then(Value::as_str) {
                targets.extend(
                    referenced_articles(text)
                        .into_iter()
                        .map(|a| Node::new(&bwb_id, a)),
                );
            }
            // The harvest's `references` block is the same statement in
            // structured form, and it carries the statute, so a cross-law
            // reference is as readable as an internal one.
            if let Some(refs) = article.get("references").and_then(Value::as_sequence) {
                for reference in refs {
                    let target_law = reference
                        .get("bwb_id")
                        .and_then(Value::as_str)
                        .unwrap_or(bwb_id.as_str());
                    match reference.get("artikel").and_then(Value::as_str) {
                        Some(artikel) => {
                            targets.insert(Node::new(target_law, artikel.to_lowercase()));
                        }
                        // A reference to a whole statute, or to a chapter or
                        // afdeling of one. It names no article, so there is
                        // nothing to walk to.
                        None if target_law != bwb_id => {
                            graph
                                .outward_law_only
                                .insert((top.clone(), target_law.to_string()));
                        }
                        None => {}
                    }
                }
            }
            targets.remove(&from);
            // An edge to an article this statute does not have is a mis-parse,
            // not a node: "artikel 99" in prose that meant another law's 99.
            // Edges into another statute are kept whatever their number,
            // because this graph cannot know that law's articles.
            targets.retain(|t| t.bwb_id != bwb_id || graph.articles_will_contain(t, &articles_seq));
            graph.depends_on.entry(from).or_default().extend(targets);
        }
        graph
    }

    /// Whether the document declares the article this node names. Consulted
    /// while scanning, so it reads the sequence rather than `self.articles`,
    /// which is still being filled.
    fn articles_will_contain(&self, node: &Node, articles: &[Value]) -> bool {
        articles.iter().any(|a| {
            entry_number(a).is_some_and(|number| top_article(&number) == node.article.as_str())
        })
    }

    /// Edges that leave this statute. Nothing in the enrich loop walks them
    /// yet; they are the input a corpus-wide order would need.
    #[must_use]
    pub fn outward_edges(&self) -> Vec<(&Node, &Node)> {
        self.depends_on
            .iter()
            .flat_map(|(from, targets)| targets.iter().map(move |to| (from, to)))
            .filter(|(_, to)| to.bwb_id != self.bwb_id)
            .collect()
    }

    /// Targets of `article` that stay inside this statute.
    fn internal_targets(&self, article: &str) -> Vec<&str> {
        self.depends_on
            .get(&Node::new(&self.bwb_id, article))
            .into_iter()
            .flatten()
            .filter(|t| t.bwb_id == self.bwb_id)
            .map(|t| t.article.as_str())
            .collect()
    }

    /// Whether two top-level articles of this statute are connected by an edge
    /// in either direction. Two articles that are not are safe to enrich side
    /// by side: neither names the other, so neither can be waiting on a name
    /// the other is about to invent.
    #[must_use]
    pub fn related(&self, a: &str, b: &str) -> bool {
        self.internal_targets(a).contains(&b) || self.internal_targets(b).contains(&a)
    }

    /// Strongly connected components of the intra-law article graph, in the
    /// order Tarjan emits them: dependencies before dependents. A component of
    /// more than one article is a cycle.
    #[must_use]
    pub fn components(&self) -> Vec<Vec<String>> {
        Tarjan::new(self).run()
    }

    /// The article graph condensed to layers: every layer is a set of
    /// components none of which depends on another in the same layer, and
    /// every layer depends only on layers before it.
    ///
    /// A cycle lands whole in one layer, which is the only honest thing to do
    /// with it: inside a component there is no order that puts producers
    /// first, so the closing pass keeps its job there.
    #[must_use]
    pub fn layers(&self) -> Vec<Vec<String>> {
        let components = self.components();
        let mut layer_of: BTreeMap<String, usize> = BTreeMap::new();
        let mut layers: Vec<Vec<String>> = Vec::new();
        for component in &components {
            let members: BTreeSet<&str> = component.iter().map(String::as_str).collect();
            // Components come out dependencies-first, so every dependency
            // outside this component already has a layer.
            let mut layer = 0usize;
            for article in component {
                for target in self.internal_targets(article) {
                    if members.contains(target) {
                        continue;
                    }
                    if let Some(l) = layer_of.get(target) {
                        layer = layer.max(l + 1);
                    }
                }
            }
            for article in component {
                layer_of.insert(article.clone(), layer);
            }
            while layers.len() <= layer {
                layers.push(Vec::new());
            }
            layers[layer].extend(component.iter().cloned());
        }
        for layer in &mut layers {
            layer.sort_by_key(|a| {
                self.articles
                    .iter()
                    .position(|x| x == a)
                    .unwrap_or(usize::MAX)
            });
        }
        layers
    }

    /// The build order the reference graph proposes: layer by layer, document
    /// order within a layer.
    ///
    /// Measured a net loss against plain document order (see the module docs),
    /// so nothing runs this by default. It stays because the measurement has
    /// to be repeatable on other laws before the conclusion generalises.
    #[must_use]
    pub fn plan_order(&self) -> Vec<String> {
        self.layers().into_iter().flatten().collect()
    }
}

/// Every `source` block under `value`, as a [`Binding`]. A walk rather than a
/// fixed path, because a source can sit in an input, a parameter or deeper,
/// and a binding the schema grows later must not drop out silently. A
/// `$`-article is chosen at run time and names nothing to follow.
fn collect_bindings(value: &Value, own_id: &str, out: &mut BTreeSet<Binding>) {
    match value {
        Value::Mapping(map) => {
            if let Some(source) = map.get("source") {
                let regulation = source
                    .get("regulation")
                    .and_then(Value::as_str)
                    .filter(|r| !r.is_empty() && *r != own_id)
                    .map(str::to_string);
                let output = source
                    .get("output")
                    .and_then(Value::as_str)
                    .filter(|o| !o.is_empty())
                    .map(str::to_string);
                let called = source
                    .get("article")
                    .and_then(Value::as_str)
                    .filter(|t| !t.starts_with('$'))
                    .and_then(|t| t.split_once('#'));
                let (regulation, article) = match called {
                    Some((law, art)) if !law.is_empty() && law != own_id => {
                        (Some(law.to_string()), Some(art.to_string()))
                    }
                    Some((_, art)) => (regulation, Some(art.to_string())),
                    None => (regulation, None),
                };
                if output.is_some() || article.is_some() {
                    out.insert(Binding {
                        regulation,
                        output,
                        article,
                    });
                }
            }
            for (_, child) in map {
                collect_bindings(child, own_id, out);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                collect_bindings(item, own_id, out);
            }
        }
        _ => {}
    }
}

/// Entry number as a string, however the YAML spelled it.
fn entry_number(article: &Value) -> Option<String> {
    article.get("number").map(|n| match n {
        Value::String(s) => s.clone(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    })
}

/// The outputs one entry declares, with the parameters its endpoint takes.
fn outputs_of(article: &Value, number: &str, index: usize) -> Vec<(String, Producer)> {
    let Some(execution) = article
        .get("machine_readable")
        .and_then(|mr| mr.get("execution"))
    else {
        return Vec::new();
    };
    let parameters: Vec<String> = execution
        .get("parameters")
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|p| p.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    execution
        .get("output")
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|o| o.get("name").and_then(Value::as_str))
                .map(|name| {
                    (
                        name.to_string(),
                        Producer {
                            entry: number.to_string(),
                            index,
                            parameters: parameters.clone(),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Iterative Tarjan over the intra-law article graph. Iterative rather than
/// recursive because a definition article of a large law can chain deeper than
/// is comfortable on a worker stack.
struct Tarjan<'a> {
    graph: &'a Graph,
    index: BTreeMap<String, usize>,
    low: BTreeMap<String, usize>,
    on_stack: BTreeSet<String>,
    stack: Vec<String>,
    counter: usize,
    out: Vec<Vec<String>>,
}

impl<'a> Tarjan<'a> {
    fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            index: BTreeMap::new(),
            low: BTreeMap::new(),
            on_stack: BTreeSet::new(),
            stack: Vec::new(),
            counter: 0,
            out: Vec::new(),
        }
    }

    fn run(mut self) -> Vec<Vec<String>> {
        for node in self.graph.articles.clone() {
            if !self.index.contains_key(&node) {
                self.strong(&node);
            }
        }
        self.out
    }

    fn strong(&mut self, root: &str) {
        let mut work: Vec<(String, usize)> = vec![(root.to_string(), 0)];
        while let Some((node, position)) = work.last().cloned() {
            if position == 0 {
                self.index.insert(node.clone(), self.counter);
                self.low.insert(node.clone(), self.counter);
                self.counter += 1;
                self.stack.push(node.clone());
                self.on_stack.insert(node.clone());
            }
            let neighbours: Vec<String> = self
                .graph
                .internal_targets(&node)
                .into_iter()
                .map(str::to_string)
                .collect();
            let mut descended = false;
            for (offset, next) in neighbours.iter().enumerate().skip(position) {
                if !self.index.contains_key(next) {
                    if let Some(last) = work.last_mut() {
                        last.1 = offset + 1;
                    }
                    work.push((next.clone(), 0));
                    descended = true;
                    break;
                } else if self.on_stack.contains(next) {
                    let candidate = self.index.get(next).copied().unwrap_or(0);
                    let current = self.low.get(&node).copied().unwrap_or(0);
                    self.low.insert(node.clone(), current.min(candidate));
                }
            }
            if descended {
                continue;
            }
            if self.low.get(&node) == self.index.get(&node) {
                let mut component = Vec::new();
                while let Some(popped) = self.stack.pop() {
                    self.on_stack.remove(&popped);
                    let done = popped == node;
                    component.push(popped);
                    if done {
                        break;
                    }
                }
                self.out.push(component);
            }
            work.pop();
            if let Some((parent, _)) = work.last().cloned() {
                let child = self.low.get(&node).copied().unwrap_or(0);
                let current = self.low.get(&parent).copied().unwrap_or(0);
                self.low.insert(parent, current.min(child));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn doc(yaml: &str) -> Value {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    #[test]
    fn top_article_strips_the_onderdeel() {
        assert_eq!(top_article("3"), "3");
        assert_eq!(top_article("3c"), "3c");
        // Numeric all the way down, so it is an article number: the corpus
        // writes `10.2a` and `2.1.1` as whole numbers and letters its leden
        // separately.
        assert_eq!(top_article("3c.1"), "3c.1");
        // `e` opens a segment with a letter, so that is where the article ends
        // and the onderdeel begins.
        assert_eq!(top_article("2.1.e.1°"), "2.1");
    }

    /// A dotted article number is an article, not an article plus a lid.
    ///
    /// Cutting on the first dot put the BRP's 141 articles on 4 nodes and the
    /// Woo's 53 on 9, which made the graph claim that articles with nothing to
    /// do with each other were the same one. Both numbers below are real: the
    /// BRP article opens with the words "Artikel 2.62", and the Wlz one carries
    /// the url `#Artikel2.1.1` with its leden lettered separately.
    #[test]
    fn a_dotted_article_number_stays_whole() {
        assert_eq!(top_article("2.62"), "2.62");
        assert_eq!(top_article("2.65"), "2.65");
        assert_eq!(top_article("2.1.1"), "2.1.1");
        assert_eq!(top_article("10.2a"), "10.2a");
        // Distinct articles stay distinct, which is the whole point.
        assert_ne!(top_article("2.62"), top_article("2.65"));
    }

    #[test]
    fn scan_records_what_each_model_reads() {
        let graph = Graph::scan(&doc(r"
$id: lezer
bwb_id: BWBR0000001
articles:
  - number: '2'
    text: Het recht.
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: awir
              output: toetsingsinkomen
          - name: premie
            source:
              output: standaardpremie
          - name: besluit
            source:
              article: awb#4:3
          - name: datum
            source:
              article: $gekozen_artikel
          - name: eigen
            source:
              regulation: lezer
              output: drempel
"));
        let binds = graph.binds_to.get("2").unwrap();
        let expected: BTreeSet<Binding> = [
            Binding {
                regulation: Some("awir".into()),
                output: Some("toetsingsinkomen".into()),
                article: None,
            },
            Binding {
                regulation: None,
                output: Some("standaardpremie".into()),
                article: None,
            },
            Binding {
                regulation: Some("awb".into()),
                output: None,
                article: Some("4:3".into()),
            },
            // Naming the own law is an intra-law binding.
            Binding {
                regulation: None,
                output: Some("drempel".into()),
                article: None,
            },
        ]
        .into_iter()
        .collect();
        assert_eq!(binds, &expected);
    }

    #[test]
    fn scan_indexes_producers_with_their_parameters() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Niets bijzonders.
  - number: '2'
    text: Niets bijzonders.
    machine_readable:
      execution:
        parameters:
          - name: bsn
        output:
          - name: is_verzekerde
"));
        let producers = graph.producers.get("is_verzekerde").unwrap();
        assert_eq!(producers.len(), 1);
        assert_eq!(producers[0].entry, "2");
        assert_eq!(producers[0].index, 1);
        assert_eq!(producers[0].parameters, vec!["bsn".to_string()]);
    }

    #[test]
    fn an_edge_needs_the_article_to_exist_in_this_law() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Onverminderd artikel 2 geldt het volgende, en artikel 99 van de Zorgverzekeringswet blijft buiten beschouwing.
  - number: '2'
    text: Niets bijzonders.
"));
        assert_eq!(graph.internal_targets("1"), vec!["2"]);
    }

    #[test]
    fn a_reference_across_the_statute_boundary_stays_an_outward_edge() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie elders.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '2'
  - number: '2'
    text: Niets bijzonders.
"));
        // Not an internal edge: article 2 of this law is not what is meant.
        assert!(graph.internal_targets("1").is_empty());
        let outward = graph.outward_edges();
        assert_eq!(outward.len(), 1);
        assert_eq!(outward[0].0.article, "1");
        assert_eq!(outward[0].1, &Node::new("BWBR0000002", "2"));
    }

    #[test]
    fn a_cycle_lands_whole_in_one_layer() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: In afwijking van artikel 2 geldt het volgende.
  - number: '2'
    text: Onverminderd artikel 1 geldt het volgende.
  - number: '3'
    text: Voor de toepassing van artikel 1 en artikel 2 geldt het volgende.
"));
        let components = graph.components();
        let cycle = components.iter().find(|c| c.len() > 1).unwrap();
        assert_eq!(
            cycle.iter().cloned().collect::<BTreeSet<_>>(),
            ["1".to_string(), "2".to_string()].into_iter().collect()
        );
        let layers = graph.layers();
        let with_one = layers
            .iter()
            .find(|l| l.contains(&"1".to_string()))
            .unwrap();
        assert!(with_one.contains(&"2".to_string()));
    }

    #[test]
    fn layers_put_a_dependency_before_its_dependent() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Het bedrag, bedoeld in artikel 2, wordt verhoogd.
  - number: '2'
    text: Het bedrag is duizend euro.
"));
        assert_eq!(
            graph.plan_order(),
            vec!["2".to_string(), "1".to_string()],
            "artikel 2 produceert wat artikel 1 leest, dus 2 gaat voor"
        );
    }

    #[test]
    fn unrelated_articles_are_not_related() {
        let graph = Graph::scan(&doc(r"
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Het bedrag, bedoeld in artikel 2, wordt verhoogd.
  - number: '2'
    text: Het bedrag is duizend euro.
  - number: '3'
    text: Deze wet treedt in werking.
"));
        assert!(graph.related("1", "2"));
        assert!(graph.related("2", "1"));
        assert!(!graph.related("1", "3"));
        assert!(!graph.related("2", "3"));
    }
}
