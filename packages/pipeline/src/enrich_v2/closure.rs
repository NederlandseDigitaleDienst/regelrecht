//! Which laws and articles one enrichment order pulls in, and where it stops.
//!
//! `--depth` counts **wetssprongen** and not artikelen. Concentric circles, and
//! the circle is the law:
//!
//! - We stand in law A at the named article. Depth 0.
//! - A step inside A costs nothing; the hot path there is followed freely.
//! - A step to law B costs a point: depth 1. Steps inside B are free again.
//! - B to C costs a point: depth 2.
//! - A straight to C also costs one point: depth 1. The depth is the distance
//!   to the law you started in and not the length of the path that found it,
//!   which makes this a breadth-first shortest distance over laws.
//!
//! Inside a law the whole law is *not* enriched. Only what lies on the hot path
//! of the article you came in through is taken. The depth bounds the number of
//! law jumps; the hot path bounds the set of articles within one law.
//!
//! ## De meting die de standaard bepaalt
//!
//! Gemeten met deze code op het corpus, vanaf `--article 69` van de
//! Zorgverzekeringswet, met de stopregels hieronder aan. De meting is gedaan
//! toen de Awb en de Awir nog als "kaderwet" naast de diepte stonden; zonder
//! die uitzondering, die inmiddels geschrapt is, komt diepte 3 op 5 396 in
//! plaats van 5 274 artikelen uit, ruim twee procent meer.
//!
//! **Elk getal hieronder is een ondergrens.** De meting is gedaan met een
//! planner die takken afkapte die binnen de diepte lagen: een wet waarvan de
//! diepte later daalde had zijn uitgaande kanten al op het oude, hogere getal
//! afgerekend, en wat daar wegviel kwam als `BeyondDepth`-gat terug in plaats
//! van als werk. Hoeveel er per diepte bij komt is niet opnieuw gemeten; dat
//! de werkelijke sluiting groter is dan de tabel, staat vast. De conclusies
//! eronder hangen aan de ordegrootte en niet aan de precieze getallen, en die
//! kant op bewegen ze niet:
//!
//! | diepte | wetten | artikelen | entries |
//! |-------:|-------:|----------:|--------:|
//! | 0      |      1 |        53 |     434 |
//! | 1      |     21 |       624 |   3 766 |
//! | 2      |     99 |     2 694 |   6 671 |
//! | 3      |    233 |     5 274 |  12 612 |
//!
//! Vanaf de zeven artikelen die de Wet op de zorgtoeslag in de Zvw aanhaalt (1,
//! 18d, 18e, 19, 24, 68b en 69) loopt het iets hoger: 55, 634, 3 369 en 6 061
//! artikelen. De ingang maakt op diepte 0 en 1 nauwelijks verschil, want de
//! vrije stap binnen de wet trekt de wet toch grotendeels mee.
//!
//! Drie dingen volgen daaruit.
//!
//! **De vrije stap binnen de wet is de uitwaaiering, niet de wetssprong.**
//! Diepte 0 is al 53 van de 86 hoofdartikelen van de Zvw, en één wetssprong
//! maakt daar 624 artikelen van. Artikel 69 haalt via de wanbetalersbepalingen
//! het Wetboek van Burgerlijke Rechtsvordering, de Faillissementswet en Boek 2
//! BW binnen, en dat heeft met zorgtoeslag niets te maken.
//!
//! **De standaard is diepte 1.** Diepte 2 is bijna drieduizend artikelen en
//! diepte 3 ruim vijfduizend, verdeeld over 233 wetten: ruim vijf procent van
//! alle wetten in het corpus. Een vangnet dat je per ongeluk aanzet en dat dan
//! de halve wetgeving verrijkt is geen vangnet.
//!
//! **Boven een grens weigert de planner.** [`Plan::refuse_above`] laat het
//! aantal artikelen zien en stopt, in plaats van een run te beginnen die dagen
//! duurt. `enrich-once` zet die grens op 200, dus zelfs diepte 1 op deze wet
//! moet met de hand worden opgehoogd. Dat is de bedoeling: het getal in de
//! weigering is de enige plek waar de omvang zichtbaar wordt vóórdat hij is
//! betaald.
//!
//! ## Stopregels, en waarom ze vóór het getal komen
//!
//! - **Delegatie.** Wijst een kant naar een ministeriële regeling of een
//!   beleidsregel, dan levert dat een open term op en houdt het daar op
//!   (RFC-026). Er zijn duizenden van die documenten en het gat dat overblijft
//!   is een bekend gat.
//! - **Buiten het corpus.** Een wet die het corpus niet heeft is een bekend gat
//!   en geen te volgen kant.
//! - **Verwijzing zonder artikel.** "de Wet langdurige zorg" of "afdeling
//!   3.3.1" noemt geen artikel, dus er is niets om naartoe te lopen. Op diepte 3
//!   staan er ruim drieduizend van; ze volgen zou het wetboek oogsten.
//!
//! ## Algemene wetten zijn gewone wetten
//!
//! Er stond hier een vierde stopregel: een handgeschreven lijst "kaderwetten"
//! (de Awb en de Awir) die als kaart naast de diepte meegingen en waar niet in
//! of uit werd gelopen. Die uitzondering is geschrapt. Gemeten op het corpus
//! komt de Awir vanaf de Wet op de zorgtoeslag op diepte 1 gewoon binnen met
//! de artikelen die op de handgeschreven kaart stonden, dus daar berekende de
//! graaf al wat de lijst wilde afdwingen, en "niet inlopen" hield nog wel de
//! producent buiten het plan die deepest-first eerder vertaald moest worden dan
//! zijn lezer. De Awb heeft het omgekeerde probleem: de zorgtoeslag citeert hem
//! zelf nergens, en de besluit-machinerie (beslistermijnen, bezwaar) komt op
//! geen enkele diepte via verwijzingen binnen; op diepte 2 is het enige
//! Awb-artikel 4:3, als bijvangst. Een kaart verscheen wel, maar alleen omdat
//! meegetrokken Zvw-artikelen toevallig een Awb-kant hebben; hij ontstond op
//! een kant die de traversal tegenkwam, hing dus af van toevallige citaten
//! elders in de sluiting, en zei niets over toepasselijkheid. Een wet die
//! werkt zonder geciteerd te worden is een relatie met een bron in de wettekst
//! (de reikwijdtebepaling, als `applies-to`-kant) en een trigger in het schema
//! (`legal_character`), geen wetscategorie in de planner. Zie RFC-026.
//!
//! ## Definitie tegenover berekening
//!
//! RFC-026 typeert kanten: `uses-definition` sleept één artikel mee,
//! `computes-with` trekt een keten. Uit de kale tekst is dat onderscheid maar
//! half te maken.
//!
//! Wat wel werkt is het **doel**: een artikel waarvan de tekst "wordt verstaan
//! onder" bevat is een begripsbepaling, en daar houdt de wandeling op.
//! [`StopRules::definitions_are_leaves`] doet dat en staat aan. In een
//! prototype van deze traversal scheelde die regel ruim een tiende van de
//! sluiting op diepte 3; met deze code is dat verschil nog niet los gemeten,
//! want er is nog geen schakelaar om hem uit te zetten.
//!
//! Wat niet werkt is de **kant**. De aanhef "bedoeld in" dekt allebei de
//! gevallen: "de persoon, bedoeld in artikel 1, onder f" is definitorisch en
//! "de premie, bedoeld in afdeling 3.3.1" vraagt een waarde. Er is geen woord
//! in de brontekst dat de twee scheidt. De diepte telt daarom over alle kanten,
//! en de consequentie staat in de tabel hierboven: diepte 1 haalt 624 artikelen
//! binnen waar een echte `computes-with`-sluiting er veel minder zou halen.
//! Het onderscheid komt terug zodra de kanten getypeerd in het corpus staan in
//! plaats van in de tekst.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

use super::refgraph::{Binding, Graph, Node, top_article};

/// Where a law lives and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawEntry {
    /// BWB identifier.
    pub bwb_id: String,
    /// `$id` of the law, as the corpus names it.
    pub law_id: String,
    /// Path of the newest version, relative to the corpus root.
    pub path: String,
    /// `regulatory_layer`, which decides whether an edge to it is delegation.
    pub layer: String,
}

impl LawEntry {
    /// Whether an edge into this law is a delegation edge, which ends the
    /// traversal with a known gap.
    #[must_use]
    pub fn is_delegated_layer(&self) -> bool {
        matches!(
            self.layer.as_str(),
            "MINISTERIELE_REGELING" | "BELEIDSREGEL" | "CIRCULAIRE"
        )
    }
}

/// Every law in a corpus, keyed by BWB identifier.
///
/// Built by reading the head of the newest version of every law directory —
/// the four fields above are all in the first dozen lines, so this never parses
/// a whole law. Four thousand directories is a fraction of a second.
#[derive(Debug, Clone, Default)]
pub struct LawIndex {
    by_bwb: BTreeMap<String, LawEntry>,
}

impl LawIndex {
    /// Scan `<root>/regulation` for law directories.
    pub fn scan(root: &Path) -> std::io::Result<Self> {
        let mut index = Self::default();
        let regulation = root.join("regulation");
        if !regulation.exists() {
            return Ok(index);
        }
        let mut stack = vec![regulation];
        while let Some(dir) = stack.pop() {
            let mut versions: Vec<PathBuf> = Vec::new();
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    stack.push(path);
                } else if is_version_file(&path) {
                    versions.push(path);
                }
            }
            versions.sort();
            let Some(newest) = versions.last() else {
                continue;
            };
            if let Some(entry) = read_head(newest, root) {
                index.by_bwb.entry(entry.bwb_id.clone()).or_insert(entry);
            }
        }
        Ok(index)
    }

    /// Look a law up by BWB identifier.
    #[must_use]
    pub fn get(&self, bwb_id: &str) -> Option<&LawEntry> {
        self.by_bwb.get(bwb_id)
    }

    /// How many laws the index holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_bwb.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_bwb.is_empty()
    }
}

/// A dated version file: `2026-01-01.yaml`. `status.yaml` and the dot-files the
/// enricher writes beside a law are not versions.
fn is_version_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.len() == "2026-01-01.yaml".len()
        && name.ends_with(".yaml")
        && name.as_bytes()[..10].iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Read `$id`, `bwb_id` and `regulatory_layer` off the head of a law file.
fn read_head(path: &Path, root: &Path) -> Option<LawEntry> {
    use std::io::BufRead;

    let file = std::fs::File::open(path).ok()?;
    let mut bwb_id = String::new();
    let mut law_id = String::new();
    let mut layer = String::new();
    for line in std::io::BufReader::new(file).lines().take(20) {
        let Ok(line) = line else { break };
        if let Some(rest) = line.strip_prefix("bwb_id:") {
            bwb_id = rest.trim().trim_matches(['\'', '"']).to_string();
        } else if let Some(rest) = line.strip_prefix("$id:") {
            law_id = rest.trim().trim_matches(['\'', '"']).to_string();
        } else if let Some(rest) = line.strip_prefix("regulatory_layer:") {
            layer = rest.trim().trim_matches(['\'', '"']).to_string();
        }
    }
    if bwb_id.is_empty() {
        return None;
    }
    Some(LawEntry {
        bwb_id,
        law_id,
        path: path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned(),
        layer,
    })
}

/// The bounds the traversal applies before the depth number does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopRules {
    /// Stop at an article whose text defines terms. See the module docs: it is
    /// the readable half of the `uses-definition` / `computes-with` split and
    /// removes about a tenth of the closure.
    pub definitions_are_leaves: bool,
}

impl Default for StopRules {
    fn default() -> Self {
        Self {
            definitions_are_leaves: true,
        }
    }
}

/// Why an edge was not followed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GapKind {
    /// The corpus does not have this law.
    OutsideCorpus,
    /// The target is a ministerial regulation or policy rule: the norm is
    /// delegated and an `open_term` is the finished answer.
    Delegated,
    /// The reference names a law, chapter or afdeling but no article.
    NoArticle,
    /// The reference names an article the target law does not have.
    ///
    /// Distinct from [`GapKind::NoArticle`], which is a limit of the reference
    /// extraction: the text named no article at all. This one is a wrong
    /// citation or a stale harvest, and those have different owners. It used to
    /// be dropped silently, so a renumbering after a wijzigingswet left no
    /// trace anywhere.
    ArticleNotInLaw,
    /// The depth ran out here.
    BeyondDepth,
}

/// One edge the traversal recorded instead of following.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gap {
    pub kind: GapKind,
    /// BWB identifier of the law the edge points at.
    pub bwb_id: String,
    /// How often this edge occurs in the closure.
    pub occurrences: usize,
}

/// One law's share of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// How many law jumps from the starting law.
    pub depth: usize,
    pub bwb_id: String,
    pub law_id: String,
    /// Path relative to the corpus root.
    pub path: String,
    /// Top-level articles on the hot path, in document order.
    pub articles: Vec<String>,
    /// Entries those articles hold, which is what an agent actually reads.
    pub entries: usize,
}

/// What `--depth` resolves to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Laws to enrich, deepest first: a producer is translated before the law
    /// that reads it, which is what makes the binding real instead of a guess.
    pub tasks: Vec<Task>,
    /// Edges recorded rather than followed.
    pub gaps: Vec<Gap>,
}

impl Plan {
    /// Total articles across every task.
    #[must_use]
    pub fn articles(&self) -> usize {
        self.tasks.iter().map(|t| t.articles.len()).sum()
    }

    /// Total entries across every task.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.tasks.iter().map(|t| t.entries).sum()
    }

    /// An error message when the plan is bigger than `limit` articles.
    ///
    /// A refusal and not a warning. Depth 2 on the Zorgverzekeringswet is four
    /// thousand articles across 156 laws; a run that starts that by accident
    /// costs days and nobody reads the warning that scrolled past.
    #[must_use]
    pub fn refuse_above(&self, limit: usize) -> Option<String> {
        if self.articles() <= limit {
            return None;
        }
        Some(format!(
            "this depth plans {} articles ({} entries) across {} laws, over the limit of {}. \
             Lower --depth, or raise the limit deliberately",
            self.articles(),
            self.entries(),
            self.tasks.len(),
            limit
        ))
    }

    /// One line per law, for the log and for the run's own record.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        self.tasks
            .iter()
            .map(|t| {
                format!(
                    "[diepte {}] {} — {} artikelen, {} entries",
                    t.depth,
                    t.law_id,
                    t.articles.len(),
                    t.entries
                )
            })
            .collect()
    }
}

/// How the traversal reached an article.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// The text or the `references` block names it.
    Cites,
    /// A model reads a value it produces, or calls its decision.
    Binds,
    /// It fills an open term of the article walked from.
    Implements,
}

/// The step that first reached an article: where from, and along what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Via {
    pub from: Node,
    pub kind: EdgeKind,
}

/// One law as the traversal needs it.
#[derive(Debug, Clone, Default)]
pub struct LoadedLaw {
    /// Its reference graph.
    pub graph: Graph,
    /// Its top-level articles that are begripsbepalingen.
    pub definitions: BTreeSet<String>,
}

/// Where the traversal gets its laws from.
///
/// The closure is the same walk whether the laws are files in a checkout
/// (`enrich-once`, [`FsLaws`]) or the facts a server holds for the corpus of a
/// traject. Only the lookups differ.
pub trait LawGraphs {
    /// What the corpus knows about a law without loading it, by BWB id;
    /// `None` when the corpus does not have it.
    fn entry(&self, bwb_id: &str) -> Option<LawEntry>;

    /// The law itself, read and scanned; `None` when it cannot be read.
    fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw>;

    /// The BWB id of the law with `$id` `law_id`, for following bindings.
    /// Without it, bindings to other laws are not followed.
    fn bwb_of(&self, _law_id: &str) -> Option<String> {
        None
    }

    /// Articles elsewhere that fill an open term of `article` in `bwb_id`.
    fn implementors(&self, _bwb_id: &str, _article: &str) -> Vec<Node> {
        Vec::new()
    }
}

/// The corpus as files under a root, as `enrich-once` reads it.
pub struct FsLaws<'a> {
    root: &'a Path,
    index: &'a LawIndex,
}

impl<'a> FsLaws<'a> {
    #[must_use]
    pub fn new(root: &'a Path, index: &'a LawIndex) -> Self {
        Self { root, index }
    }
}

impl LawGraphs for FsLaws<'_> {
    fn entry(&self, bwb_id: &str) -> Option<LawEntry> {
        self.index.get(bwb_id).cloned()
    }

    fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw> {
        let entry = self.index.get(bwb_id)?;
        let doc = read_law(&self.root.join(&entry.path))?;
        Some(loaded(&doc))
    }
}

/// Scan a parsed law into what the traversal needs.
#[must_use]
pub fn loaded(doc: &Value) -> LoadedLaw {
    let graph = Graph::scan(doc);
    let definitions = graph
        .articles
        .iter()
        .filter(|a| is_definition_article(doc, a))
        .cloned()
        .collect();
    LoadedLaw { graph, definitions }
}

/// What the walk is allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walk {
    /// Law jumps from the starting law.
    pub depth: usize,
    pub rules: StopRules,
    /// Follow what the models read ([`Graph::binds_to`]) and the articles
    /// that fill an open term, besides what the text cites. A binding or an
    /// implements edge is followed into a delegated regulation, which a
    /// citation is not: the value read there is part of the computation.
    pub follow_bindings: bool,
    /// Stop taking articles once this many are taken.
    pub node_cap: Option<usize>,
}

/// What one walk found, before it is turned into a plan or a chain.
#[derive(Debug, Clone, Default)]
pub struct Closure {
    /// Taken articles per law, by BWB id.
    pub taken: BTreeMap<String, BTreeSet<String>>,
    /// Law jumps from the start, per law.
    pub law_depth: BTreeMap<String, usize>,
    /// Which law reads which, for ordering producers first.
    pub reads: BTreeMap<String, BTreeSet<String>>,
    /// The scanned laws the walk loaded.
    pub laws: BTreeMap<String, Graph>,
    /// How each article was first reached. The start articles have none.
    pub via: BTreeMap<Node, Via>,
    /// Edges recorded rather than followed, with how often.
    pub gaps: BTreeMap<(GapKind, String), usize>,
    /// Whether the node cap stopped the walk.
    pub node_capped: bool,
}

/// One edge leaving the free walk of a ring, charged after the ring.
struct Crossing {
    from: Node,
    target_law: String,
    target_article: String,
    kind: EdgeKind,
}

/// Walk the closure from `start_articles` of the law `start_bwb`.
///
/// The ring discipline and the depth rules are described at
/// [`plan_closure`]; this is that walk over any [`LawGraphs`].
///
/// # Errors
///
/// When the starting law cannot be loaded, or does not have one of the
/// start articles.
pub fn walk_closure<G: LawGraphs>(
    graphs: &mut G,
    start_bwb: &str,
    start_articles: &[String],
    walk: Walk,
) -> std::result::Result<Closure, String> {
    let start = graphs
        .load(start_bwb)
        .ok_or_else(|| format!("cannot read the law {start_bwb}"))?;
    for article in start_articles {
        if !start.graph.articles.iter().any(|a| a == article) {
            return Err(format!(
                "law {start_bwb} has no article {article}; naming one it does not have is a \
                 mistake in the query and a plan of nothing would look like a plan of nothing to do"
            ));
        }
    }
    let mut walker = Walker {
        graphs,
        walk,
        definitions: BTreeMap::new(),
        out: Closure::default(),
        pending: BTreeMap::new(),
    };
    walker.out.laws.insert(start_bwb.to_string(), start.graph);
    walker
        .definitions
        .insert(start_bwb.to_string(), start.definitions);
    walker.out.law_depth.insert(start_bwb.to_string(), 0);
    walker.run(start_bwb, start_articles);
    Ok(walker.out)
}

struct Walker<'g, G: LawGraphs> {
    graphs: &'g mut G,
    walk: Walk,
    definitions: BTreeMap<String, BTreeSet<String>>,
    out: Closure,
    /// Articles waiting to be walked, bucketed by the depth they sit at. A
    /// single ring advanced by one does not hold: an article that surfaces
    /// late on an already-known law re-enters at that law's own, shallower
    /// depth.
    pending: BTreeMap<usize, Vec<(String, String)>>,
}

impl<G: LawGraphs> Walker<'_, G> {
    fn gap(&mut self, kind: GapKind, law: &str) {
        *self.out.gaps.entry((kind, law.to_string())).or_default() += 1;
    }

    /// Take an article, unless it was taken already or the cap is reached.
    /// Returns whether it is new.
    fn take(&mut self, law: &str, article: &str, via: Option<Via>) -> bool {
        if self
            .out
            .taken
            .get(law)
            .is_some_and(|set| set.contains(article))
        {
            return false;
        }
        if let Some(cap) = self.walk.node_cap {
            let taken: usize = self.out.taken.values().map(BTreeSet::len).sum();
            if taken >= cap {
                self.out.node_capped = true;
                return false;
            }
        }
        self.out
            .taken
            .entry(law.to_string())
            .or_default()
            .insert(article.to_string());
        if let Some(via) = via {
            self.out.via.entry(Node::new(law, article)).or_insert(via);
        }
        true
    }

    /// Make sure a law is loaded; `false` when it cannot be read.
    fn ensure_loaded(&mut self, bwb: &str) -> bool {
        if self.out.laws.contains_key(bwb) {
            return true;
        }
        let Some(law) = self.graphs.load(bwb) else {
            return false;
        };
        self.out.laws.insert(bwb.to_string(), law.graph);
        self.definitions.insert(bwb.to_string(), law.definitions);
        true
    }

    fn run(&mut self, start_bwb: &str, start_articles: &[String]) {
        // The articles this ring came in through. An entry point is an
        // article the closure was pointed at; a definition article is a leaf
        // only when it was walked into, never when it was asked for by name.
        let mut ring: Vec<(String, String)> = Vec::new();
        for article in start_articles {
            if self.take(start_bwb, article, None) {
                ring.push((start_bwb.to_string(), article.clone()));
            }
        }
        let mut here = 0usize;
        while !ring.is_empty() {
            let crossings = self.free_walk(&ring);
            for crossing in crossings {
                self.charge(crossing, here);
            }
            // The next ring is the shallowest bucket with work left, which is
            // not always `here + 1`: a late article re-enters at its own law's
            // depth.
            let Some(next_depth) = self.pending.keys().copied().next() else {
                break;
            };
            ring = self.pending.remove(&next_depth).unwrap_or_default();
            here = next_depth;
        }
    }

    /// The free walk inside the laws of one ring, to closure. Returns the
    /// edges that leave a law, to be charged once the ring is walked out:
    /// following one on sight would put a law of the next ring in front of
    /// the articles of this ring that have not run yet.
    fn free_walk(&mut self, ring: &[(String, String)]) -> Vec<Crossing> {
        let mut crossings: Vec<Crossing> = Vec::new();
        let mut queue: VecDeque<(String, String, bool)> = ring
            .iter()
            .map(|(l, a)| (l.clone(), a.clone(), true))
            .collect();
        while let Some((bwb, article, entry_point)) = queue.pop_front() {
            let Some(graph) = self.out.laws.get(&bwb) else {
                continue;
            };
            if self.walk.rules.definitions_are_leaves
                && !entry_point
                && self
                    .definitions
                    .get(&bwb)
                    .is_some_and(|d| d.contains(&article))
            {
                continue;
            }
            let from = Node::new(&bwb, &article);
            let mut internal: Vec<(String, EdgeKind)> = Vec::new();
            if let Some(targets) = graph.depends_on.get(&from) {
                for target in targets {
                    if target.bwb_id == bwb {
                        internal.push((target.article.clone(), EdgeKind::Cites));
                    } else {
                        crossings.push(Crossing {
                            from: from.clone(),
                            target_law: target.bwb_id.clone(),
                            target_article: target.article.clone(),
                            kind: EdgeKind::Cites,
                        });
                    }
                }
            }
            let no_article: Vec<String> = graph
                .outward_law_only
                .iter()
                .filter(|(f, _)| *f == article)
                .map(|(_, law)| law.clone())
                .collect();
            let bindings: Vec<Binding> = if self.walk.follow_bindings {
                graph
                    .binds_to
                    .get(&article)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            for law in no_article {
                self.gap(GapKind::NoArticle, &law);
            }
            if self.walk.follow_bindings {
                for binding in bindings {
                    self.binding_edge(&from, &binding, &mut internal, &mut crossings);
                }
                for target in self.graphs.implementors(&bwb, &article) {
                    if target.bwb_id == bwb {
                        internal.push((target.article, EdgeKind::Implements));
                    } else {
                        crossings.push(Crossing {
                            from: from.clone(),
                            target_law: target.bwb_id,
                            target_article: target.article,
                            kind: EdgeKind::Implements,
                        });
                    }
                }
            }
            // Free inside the law.
            for (target, kind) in internal {
                let via = Via {
                    from: from.clone(),
                    kind,
                };
                if self.take(&bwb, &target, Some(via)) {
                    queue.push_back((bwb.clone(), target, false));
                }
            }
        }
        crossings
    }

    /// Turn one binding into an edge: inside the law through its own
    /// producers, or out through the producers of the law it names. A binding
    /// that resolves to nothing is left alone; the facts report it.
    fn binding_edge(
        &mut self,
        from: &Node,
        binding: &Binding,
        internal: &mut Vec<(String, EdgeKind)>,
        crossings: &mut Vec<Crossing>,
    ) {
        let target_law = match &binding.regulation {
            None => from.bwb_id.clone(),
            Some(law_id) => match self.graphs.bwb_of(law_id) {
                Some(bwb) => bwb,
                None => {
                    self.gap(GapKind::OutsideCorpus, law_id);
                    return;
                }
            },
        };
        let article = match (&binding.article, &binding.output) {
            (Some(article), _) => Some(top_article(article).to_string()),
            (None, Some(output)) => {
                if !self.ensure_loaded(&target_law) {
                    return;
                }
                self.out
                    .laws
                    .get(&target_law)
                    .and_then(|g| g.producers.get(output))
                    .and_then(|p| p.first())
                    .map(|p| top_article(&p.entry).to_string())
            }
            (None, None) => None,
        };
        let Some(article) = article else {
            return;
        };
        if target_law == from.bwb_id {
            let exists = self
                .out
                .laws
                .get(&target_law)
                .is_some_and(|g| g.articles.contains(&article));
            if !exists {
                self.gap(GapKind::ArticleNotInLaw, &target_law);
            } else if article != from.article {
                internal.push((article, EdgeKind::Binds));
            }
        } else {
            crossings.push(Crossing {
                from: from.clone(),
                target_law,
                target_article: article,
                kind: EdgeKind::Binds,
            });
        }
    }

    /// Charge one outward edge against the depth of the law it leaves from.
    ///
    /// Charging every edge at `here + 1` was wrong for an article that
    /// surfaces late: a new article on a law already known at a shallow depth
    /// was collected on a deep ring, and its own edges were then billed for
    /// every jump that ring had cost.
    fn charge(&mut self, crossing: Crossing, here: usize) {
        let Crossing {
            from,
            target_law,
            target_article,
            kind,
        } = crossing;
        let from_depth = self
            .out
            .law_depth
            .get(&from.bwb_id)
            .copied()
            .unwrap_or(here);
        let there = from_depth + 1;

        let Some(entry) = self.graphs.entry(&target_law) else {
            self.gap(GapKind::OutsideCorpus, &target_law);
            return;
        };
        // A citation into a delegated regulation ends at an open term. A
        // binding or an implements edge into one is followed: the value read
        // there is part of the computation.
        if entry.is_delegated_layer() && kind == EdgeKind::Cites {
            self.gap(GapKind::Delegated, &target_law);
            return;
        }
        if !self.ensure_loaded(&target_law) {
            self.gap(GapKind::OutsideCorpus, &target_law);
            return;
        }
        // An article the target law does not have used to vanish here
        // without a word. Checked before anything is recorded about the law,
        // so a bogus citation neither writes a depth nor a read edge.
        let exists = self
            .out
            .laws
            .get(&target_law)
            .is_some_and(|g| g.articles.contains(&target_article));
        if !exists {
            self.gap(GapKind::ArticleNotInLaw, &target_law);
            return;
        }

        // A law new to the walk is taken only within the depth. A law already
        // in it takes any further article, and a shorter route found later
        // wins: its depth only ever falls. A settled depth never exceeds the
        // limit, so neither does the lower one.
        match self.out.law_depth.get_mut(&target_law) {
            Some(settled) => *settled = (*settled).min(there),
            None if there > self.walk.depth => {
                self.gap(GapKind::BeyondDepth, &target_law);
                return;
            }
            None => {
                self.out.law_depth.insert(target_law.clone(), there);
            }
        }

        // `reads` is written once the edge is known to lead somewhere real; a
        // citation to an article that does not exist must not create an
        // ordering constraint.
        self.out
            .reads
            .entry(from.bwb_id.clone())
            .or_default()
            .insert(target_law.clone());

        let via = Via { from, kind };
        if self.take(&target_law, &target_article, Some(via)) {
            let depth = self
                .out
                .law_depth
                .get(&target_law)
                .copied()
                .unwrap_or(there);
            self.pending
                .entry(depth)
                .or_default()
                .push((target_law, target_article));
        }
    }
}

/// Plan the closure from one article of one law.
///
/// `start_path` is relative to `root`. Errors only when the starting law cannot
/// be read; everything else the traversal meets and cannot follow becomes a
/// gap, because a closure that fails on the first unharvested law would never
/// produce a plan at all.
///
/// The depth per law comes from a strict breadth-first walk over laws, one
/// whole ring at a time ([`walk_closure`]). The ring is the unit because the
/// two kinds of step cost different things. A step inside a law is free, a
/// step to another law costs a point, and a single queue holding both mixes
/// them: a free step taken now and a paid step taken now come back out in the
/// order they went in, which is arrival order and not distance. That is what a
/// flat FIFO got wrong. It charged an article's outward edges against whatever
/// depth its law happened to carry at the moment that article was dequeued,
/// and a law whose depth dropped later had already spent the higher number:
/// a law within `--depth` vanished from the plan and came back as a known gap.
///
/// So the depth of every law in ring `d` is settled before any article of ring
/// `d` is looked at. Within a ring the free steps run to closure first, and
/// only then are the outward edges of the whole ring charged together.
///
/// # Errors
///
/// When the starting law cannot be read or lacks a start article.
pub fn plan_closure(
    root: &Path,
    start_path: &str,
    start_articles: &[String],
    depth: usize,
    index: &LawIndex,
    rules: StopRules,
) -> std::result::Result<Plan, String> {
    let start_doc = read_law(&root.join(start_path))
        .ok_or_else(|| format!("cannot read the law at {start_path}"))?;
    let start_bwb = start_doc
        .get("bwb_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut laws = StartAware {
        inner: FsLaws::new(root, index),
        start_bwb: start_bwb.clone(),
        start: Some(loaded(&start_doc)),
    };
    let walk = Walk {
        depth,
        rules,
        follow_bindings: false,
        node_cap: None,
    };
    let closure = walk_closure(&mut laws, &start_bwb, start_articles, walk).map_err(|e| {
        e.replacen(
            &format!("law {start_bwb} "),
            &format!("law {start_path} "),
            1,
        )
    })?;
    Ok(plan_of(closure, index))
}

/// [`FsLaws`] with the starting law already read: it is named by path, and
/// the index may not have it.
struct StartAware<'a> {
    inner: FsLaws<'a>,
    start_bwb: String,
    start: Option<LoadedLaw>,
}

impl LawGraphs for StartAware<'_> {
    fn entry(&self, bwb_id: &str) -> Option<LawEntry> {
        self.inner.entry(bwb_id)
    }

    fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw> {
        if bwb_id == self.start_bwb
            && let Some(start) = self.start.take()
        {
            return Some(start);
        }
        self.inner.load(bwb_id)
    }
}

/// Order the walk's laws into tasks, producers before readers.
fn plan_of(closure: Closure, index: &LawIndex) -> Plan {
    let Closure {
        taken,
        law_depth,
        reads,
        laws,
        gaps,
        ..
    } = closure;
    let mut tasks: Vec<Task> = Vec::new();
    for (bwb, articles) in taken {
        let Some(graph) = laws.get(&bwb) else {
            continue;
        };
        let entry = index.get(&bwb);
        let ordered: Vec<String> = graph
            .articles
            .iter()
            .filter(|a| articles.contains(*a))
            .cloned()
            .collect();
        let entries = graph
            .entries
            .iter()
            .filter(|e| ordered.iter().any(|a| a == top_article(e)))
            .count();
        tasks.push(Task {
            depth: law_depth.get(&bwb).copied().unwrap_or(0),
            law_id: entry.map_or_else(String::new, |e| e.law_id.clone()),
            path: entry.map_or_else(String::new, |e| e.path.clone()),
            bwb_id: bwb,
            articles: ordered,
            entries,
        });
    }
    // Producers before readers, over the whole plan rather than within one
    // depth. Depth alone cannot carry this: a law reached shallowly through
    // one article can be read through *another* article by a law that sits
    // deeper, and then the deeper task comes first while the article it reads
    // has not been translated. A comparison function cannot express it either,
    // because "reads" is not a total order and sorting on it silently produces
    // whatever the algorithm's pairwise choices happen to give.
    //
    // So: a topological walk over the reads graph, with depth as the tiebreak
    // so the deepest of the currently-free tasks goes first, and the name
    // after that so the result is stable. A cycle leaves nobody free; those
    // tasks come out in depth-then-name order and the closing pass connects
    // what no order could.
    tasks.sort_by(|a, b| b.depth.cmp(&a.depth).then(a.law_id.cmp(&b.law_id)));
    let mut ordered: Vec<Task> = Vec::with_capacity(tasks.len());
    let mut placed: BTreeSet<String> = BTreeSet::new();
    while !tasks.is_empty() {
        // Free: everything it reads is either already placed or not in the
        // plan at all (a known gap, which nothing here can produce).
        let next = tasks.iter().position(|t| {
            reads.get(&t.bwb_id).is_none_or(|targets| {
                targets.iter().all(|target| {
                    *target == t.bwb_id
                        || placed.contains(target)
                        || !tasks.iter().any(|other| other.bwb_id == *target)
                })
            })
        });
        // Nothing free means every remaining task sits in a cycle. Take the
        // first in the existing order rather than looping forever.
        let task = tasks.remove(next.unwrap_or(0));
        placed.insert(task.bwb_id.clone());
        ordered.push(task);
    }

    Plan {
        tasks: ordered,
        gaps: gaps
            .into_iter()
            .map(|((kind, bwb_id), occurrences)| Gap {
                kind,
                bwb_id,
                occurrences,
            })
            .collect(),
    }
}

fn read_law(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_yaml_ng::from_str(&raw).ok()
}

/// Words a definition provision uses. The same list `checks` uses, because a
/// begripsbepaling is one thing and two definitions of it would drift.
const DEFINITION_WORDS: &[&str] = &[
    "wordt verstaan onder",
    "verstaan onder:",
    "wordt in deze",
    "wordt in dit",
];

fn is_definition_article(doc: &Value, article: &str) -> bool {
    let mut text = String::new();
    for entry in doc
        .get("articles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        if super::refgraph::top_article(number) != article {
            continue;
        }
        if let Some(t) = entry.get("text").and_then(Value::as_str) {
            text.push_str(&t.to_lowercase());
            text.push(' ');
        }
    }
    DEFINITION_WORDS.iter().any(|w| text.contains(w))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Three laws: A reads B and C straight, B reads C. C is therefore at
    /// depth 1 and not at 2, which is the rule that makes the depth a distance
    /// and not a path length.
    fn corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De hoogte volgt uit artikel 5 van wet B en uit artikel 9 van wet C.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '5'
      - id: ref2
        bwb_id: BWBR0000003
        artikel: '9'
  - number: '2'
    text: Een artikel dat niemand aanhaalt.
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_b/2026-01-01.yaml",
            r"$id: wet_b
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '5'
    text: Het bedrag wordt berekend met artikel 6 en met artikel 9 van wet C.
    references:
      - id: ref1
        bwb_id: BWBR0000003
        artikel: '9'
  - number: '6'
    text: Het tarief is tien procent.
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_c/2026-01-01.yaml",
            r"$id: wet_c
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '9'
    text: Het percentage wordt bij ministeriële regeling vastgesteld.
",
        );
        dir
    }

    fn write(dir: &tempfile::TempDir, rel: &str, body: &str) {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn plan_of(dir: &tempfile::TempDir, depth: usize) -> Plan {
        let index = LawIndex::scan(dir.path()).unwrap();
        plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["1".to_string()],
            depth,
            &index,
            StopRules::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_index_finds_every_law_by_its_bwb_number() {
        let dir = corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        assert_eq!(index.len(), 3);
        assert_eq!(index.get("BWBR0000002").unwrap().law_id, "wet_b");
        assert_eq!(
            index.get("BWBR0000002").unwrap().path,
            "regulation/nl/wet/wet_b/2026-01-01.yaml"
        );
    }

    #[test]
    fn depth_zero_stays_inside_the_starting_law() {
        let plan = plan_of(&corpus(), 0);
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].law_id, "wet_a");
        assert_eq!(plan.tasks[0].articles, vec!["1".to_string()]);
        // Article 2 is in the same law but not on the hot path: a step inside
        // a law is free, it is not automatic.
        assert!(!plan.tasks[0].articles.contains(&"2".to_string()));
        // Both outward edges are recorded as beyond the depth.
        assert_eq!(plan.gaps.len(), 2);
        assert!(plan.gaps.iter().all(|g| g.kind == GapKind::BeyondDepth));
    }

    /// The rule that gives the depth its meaning: A reaches C straight, so C
    /// costs one point, whatever the longer route through B would cost.
    #[test]
    fn a_law_reached_straight_from_the_start_costs_one_jump() {
        let plan = plan_of(&corpus(), 1);
        let depth_of = |id: &str| {
            plan.tasks
                .iter()
                .find(|t| t.law_id == id)
                .map(|t| t.depth)
                .unwrap()
        };
        assert_eq!(depth_of("wet_a"), 0);
        assert_eq!(depth_of("wet_b"), 1);
        assert_eq!(depth_of("wet_c"), 1, "A haalt C rechtstreeks, dus niveau 1");
    }

    /// Inside a law only the hot path is taken. Article 6 of B comes along
    /// because article 5 names it; article 2 of A stays out.
    #[test]
    fn a_step_inside_a_law_is_free_but_only_along_the_hot_path() {
        let plan = plan_of(&corpus(), 1);
        let b = plan.tasks.iter().find(|t| t.law_id == "wet_b").unwrap();
        assert_eq!(b.articles, vec!["5".to_string(), "6".to_string()]);
        let a = plan.tasks.iter().find(|t| t.law_id == "wet_a").unwrap();
        assert_eq!(a.articles, vec!["1".to_string()]);
    }

    /// Deepest first: wet_b and wet_c are translated before wet_a, so wet_a
    /// binds to names that already exist.
    #[test]
    fn the_plan_puts_producers_before_their_readers() {
        let plan = plan_of(&corpus(), 1);
        let depths: Vec<usize> = plan.tasks.iter().map(|t| t.depth).collect();
        assert_eq!(depths, vec![1, 1, 0]);
    }

    /// A law that declares itself applicable is an ordinary law in the plan.
    /// There used to be a designated list ("kaderwetten") whose members came
    /// along as a card beside the depth and were never walked into; a
    /// referenced producer must instead land in the plan as a task, deepest
    /// first, like any other law.
    #[test]
    fn a_self_declaring_law_is_an_ordinary_task_in_the_plan() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 3 van de algemene wet.
    references:
      - id: ref1
        bwb_id: BWBR0000009
        artikel: '3'
",
        );
        write(
            &dir,
            "regulation/nl/wet/algemene_wet/2026-01-01.yaml",
            r"$id: algemene_wet
regulatory_layer: WET
bwb_id: BWBR0000009
articles:
  - number: '3'
    text: Deze wet is van toepassing op elk besluit.
",
        );
        let plan = plan_of(&dir, 1);
        let task = plan
            .tasks
            .iter()
            .find(|t| t.law_id == "algemene_wet")
            .expect("de aangehaalde wet staat als taak in het plan");
        assert_eq!(task.depth, 1, "een wetssprong kost een punt, ook hier");
        assert_eq!(task.articles, vec!["3".to_string()]);
    }

    #[test]
    fn a_delegated_layer_ends_the_traversal_as_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 2 van de regeling.
    references:
      - id: ref1
        bwb_id: BWBR0000008
        artikel: '2'
",
        );
        write(
            &dir,
            "regulation/nl/wet/de_regeling/2026-01-01.yaml",
            r"$id: de_regeling
regulatory_layer: MINISTERIELE_REGELING
bwb_id: BWBR0000008
articles:
  - number: '2'
    text: Het percentage is drie.
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.gaps.len(), 1);
        assert_eq!(plan.gaps[0].kind, GapKind::Delegated);
    }

    #[test]
    fn a_law_the_corpus_does_not_have_is_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie artikel 2 van een wet die hier niet staat.
    references:
      - id: ref1
        bwb_id: BWBR9999999
        artikel: '2'
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.gaps[0].kind, GapKind::OutsideCorpus);
        assert_eq!(plan.gaps[0].bwb_id, "BWBR9999999");
    }

    /// A reference that names a law but no article has nothing to walk to. On
    /// the real corpus at depth 3 there are over three thousand of these.
    #[test]
    fn a_reference_without_an_article_is_a_known_gap() {
        let dir = corpus();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Zie de Wet langdurige zorg.
    references:
      - id: ref1
        bwb_id: BWBR0000002
",
        );
        let plan = plan_of(&dir, 3);
        assert_eq!(plan.gaps.len(), 1);
        assert_eq!(plan.gaps[0].kind, GapKind::NoArticle);
        assert_eq!(plan.tasks.len(), 1);
    }

    #[test]
    fn a_definition_article_is_a_leaf_when_walked_into() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: In deze wet wordt verstaan onder het begrip partner hetgeen artikel 4 bepaalt.
  - number: '2'
    text: De aanspraak volgt uit artikel 1.
  - number: '4'
    text: Een artikel dat alleen via de begripsbepaling bereikbaar is.
",
        );
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = |rules| {
            plan_closure(
                dir.path(),
                "regulation/nl/wet/wet_a/2026-01-01.yaml",
                &["2".to_string()],
                1,
                &index,
                rules,
            )
            .unwrap()
        };
        let stopping = plan(StopRules {
            definitions_are_leaves: true,
        });
        assert_eq!(
            stopping.tasks[0].articles,
            vec!["1".to_string(), "2".to_string()],
            "de begripsbepaling komt mee, maar sleept artikel 4 niet mee"
        );
        let walking = plan(StopRules {
            definitions_are_leaves: false,
        });
        assert_eq!(
            walking.tasks[0].articles,
            vec!["1".to_string(), "2".to_string(), "4".to_string()]
        );
    }

    /// Naming an article the law does not have fails loudly: a plan of nothing
    /// is indistinguishable from a plan with nothing to do.
    #[test]
    fn an_article_the_law_does_not_have_fails_the_plan() {
        let dir = corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        let error = plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["99".to_string()],
            1,
            &index,
            StopRules::default(),
        )
        .unwrap_err();
        assert!(error.contains("no article 99"), "{error}");
    }

    /// A law within the depth must not have its children refused as if it lay
    /// outside it.
    ///
    /// The corpus below is the shape that broke: wet_a walks a free chain of
    /// its own articles (1 → 2 → 3 → 3a → 4), and the long route to wet_d leaves
    /// from article 2 while the short one leaves from article 4. A flat queue
    /// mixing free steps and law jumps dequeues wet_b before it ever reaches
    /// article 4, so wet_d is first recorded at depth 2 and its edge to wet_e
    /// is charged at 3 and refused. Article 4 then lowers wet_d to 1, too late
    /// for the edge that was already dropped, and `taken` blocks a requeue.
    ///
    /// The plan that came out said wet_d sits at depth 1 and reported wet_e as
    /// beyond a depth of 2, which cannot both be true. At `--depth 2` wet_e is
    /// two jumps out and belongs in the plan.
    fn overtaking_corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De aanspraak volgt uit artikel 2.
  - number: '2'
    text: De hoogte volgt uit artikel 3 en uit artikel 5 van wet B.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '5'
  - number: '3'
    text: Het tarief volgt uit artikel 3a.
  - number: '3a'
    text: De maatstaf volgt uit artikel 4.
  - number: '4'
    text: De grondslag is die van artikel 1 van wet D.
    references:
      - id: ref2
        bwb_id: BWBR0000004
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_b/2026-01-01.yaml",
            r"$id: wet_b
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '5'
    text: Het bedrag is dat van artikel 1 van wet D.
    references:
      - id: ref1
        bwb_id: BWBR0000004
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_d/2026-01-01.yaml",
            r"$id: wet_d
regulatory_layer: WET
bwb_id: BWBR0000004
articles:
  - number: '1'
    text: Het percentage is dat van artikel 7 van wet E.
    references:
      - id: ref1
        bwb_id: BWBR0000005
        artikel: '7'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_e/2026-01-01.yaml",
            r"$id: wet_e
regulatory_layer: WET
bwb_id: BWBR0000005
articles:
  - number: '7'
    text: Het percentage is tien.
",
        );
        dir
    }

    /// Two laws one jump out, where one reads the other. Depth cannot order
    /// them and the alphabetical fallback put the reader first: `aaa_leest`
    /// was planned before `mmm_levert`, whose output it binds to, which is
    /// exactly what "deepest first, producer before reader" promises does not
    /// happen. The name still decides between laws that do not read each
    /// other, so the order stays stable.
    fn tie_corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wortel/2026-01-01.yaml",
            r"$id: wortel
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Volgt uit artikel 1 van aaa en artikel 1 van mmm.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '1'
      - id: ref2
        bwb_id: BWBR0000003
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/aaa_leest/2026-01-01.yaml",
            r"$id: aaa_leest
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '1'
    text: Het bedrag is dat van artikel 1 van mmm.
    references:
      - id: ref3
        bwb_id: BWBR0000003
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/mmm_levert/2026-01-01.yaml",
            r"$id: mmm_levert
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '1'
    text: Het bedrag bedraagt honderd euro.
",
        );
        dir
    }

    fn late_article_corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Volgt uit artikel 1 van wet P en artikel 1 van wet X.
    references:
      - id: r1
        bwb_id: BWBR0000002
        artikel: '1'
      - id: r2
        bwb_id: BWBR0000003
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_p/2026-01-01.yaml",
            r"$id: wet_p
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '1'
    text: Volgt uit artikel 1 van wet Q.
    references:
      - id: r3
        bwb_id: BWBR0000004
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_q/2026-01-01.yaml",
            r"$id: wet_q
regulatory_layer: WET
bwb_id: BWBR0000004
articles:
  - number: '1'
    text: Volgt uit artikel 9 van wet X.
    references:
      - id: r4
        bwb_id: BWBR0000003
        artikel: '9'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_x/2026-01-01.yaml",
            r"$id: wet_x
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '1'
    text: Het eerste artikel.
  - number: '9'
    text: Het negende artikel levert de waarde die wet Q leest.
",
        );
        dir
    }

    /// A law reached shallowly through one article, and read through *another*
    /// article by a law that sits deeper.
    ///
    /// `wet_a` cites `wet_p` and `wet_x` directly (depth 1), `wet_p` cites
    /// `wet_q` (depth 2), and `wet_q` reads article 9 of `wet_x` — an article
    /// nobody had taken yet on a law that was already known. `wet_x` keeps its
    /// depth 1, correctly, and deepest-first then put `wet_q` in front of the
    /// producer of the very article it reads. Depth cannot carry the ordering
    /// guarantee on its own, so the plan is walked topologically over the
    /// reads graph with depth as the tiebreak.
    #[test]
    fn a_law_read_through_a_later_article_is_still_planned_first() {
        let dir = late_article_corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["1".to_string()],
            3,
            &index,
            StopRules::default(),
        )
        .unwrap();
        let position = |id: &str| {
            plan.tasks
                .iter()
                .position(|t| t.law_id == id)
                .unwrap_or_else(|| panic!("{id} ontbreekt: {:?}", plan.describe()))
        };
        assert!(
            position("wet_x") < position("wet_q"),
            "wet_q leest artikel 9 van wet_x, dus wet_x hoort eerst: {:?}",
            plan.describe()
        );
        assert_eq!(
            plan.tasks
                .iter()
                .find(|t| t.law_id == "wet_x")
                .map(|t| t.depth),
            Some(1),
            "de diepte van wet_x mag niet verschoven zijn"
        );
    }

    /// A corpus where the root itself gains an article on a deep ring.
    ///
    /// `wortel` art 1 reads `wet_m`, which reads `wet_n`, which points back at
    /// `wortel` art 50. That article reads `wet_z`, so `wet_z` sits one jump
    /// from the root: the article it is read from belongs to the root.
    fn late_root_article_corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wortel/2026-01-01.yaml",
            r"$id: wortel
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Volgt uit artikel 1 van wet M.
    references:
      - id: r1
        bwb_id: BWBR0000002
        artikel: '1'
  - number: '50'
    text: Volgt uit artikel 1 van wet Z.
    references:
      - id: r2
        bwb_id: BWBR0000004
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_m/2026-01-01.yaml",
            r"$id: wet_m
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '1'
    text: Volgt uit artikel 1 van wet N.
    references:
      - id: r3
        bwb_id: BWBR0000003
        artikel: '1'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_n/2026-01-01.yaml",
            r"$id: wet_n
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '1'
    text: Volgt uit artikel 50 van de wortel.
    references:
      - id: r4
        bwb_id: BWBR0000001
        artikel: '50'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_z/2026-01-01.yaml",
            r"$id: wet_z
regulatory_layer: WET
bwb_id: BWBR0000004
articles:
  - number: '1'
    text: Sluitstuk.
",
        );
        dir
    }

    /// An edge is charged against the law it leaves from, not the ring it was
    /// collected in.
    ///
    /// Every outward edge of a ring used to be billed at `here + 1`. An article
    /// that surfaces late on an already-known law is collected on a deep ring,
    /// so its own edges were charged for jumps that article never made. Here
    /// `wet_z` is read straight from an article of the root, one jump away, and
    /// it was reported as a `BeyondDepth` gap at depth 2 and 3 while the plan
    /// listed the root at depth 0 with that very article in it.
    #[test]
    fn an_edge_costs_what_its_own_law_is_deep() {
        let dir = late_root_article_corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        // From depth 2 on, because the route that surfaces article 50 runs
        // through wet_n at depth 2. At depth 1 that article is never seen at
        // all, which is the limit doing its job rather than the bug.
        for depth in 2..=4 {
            let plan = plan_closure(
                dir.path(),
                "regulation/nl/wet/wortel/2026-01-01.yaml",
                &["1".to_string()],
                depth,
                &index,
                StopRules::default(),
            )
            .unwrap();
            let z = plan.tasks.iter().find(|t| t.law_id == "wet_z");
            assert_eq!(
                z.map(|t| t.depth),
                Some(1),
                "wet_z ligt op één sprong van de wortel, bij --depth {depth}: {:?}",
                plan.describe()
            );
            assert!(
                !plan
                    .gaps
                    .iter()
                    .any(|g| g.kind == GapKind::BeyondDepth && g.bwb_id == "BWBR0000004"),
                "en is dus geen BeyondDepth-gat bij --depth {depth}: {:?}",
                plan.describe()
            );
        }
    }

    /// A citation to an article the target law does not have leaves a gap.
    ///
    /// It used to be a bare `continue`: no task, no gap, no trace. A wrong
    /// article number is what a renumbering after a wijzigingswet or a slip in
    /// the harvest looks like, so it is exactly the kind of thing an
    /// enrichment run exists to surface.
    #[test]
    fn a_citation_to_an_article_that_does_not_exist_is_a_known_gap() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: Volgt uit artikel 99 van wet B.
    references:
      - id: r1
        bwb_id: BWBR0000002
        artikel: '99'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_b/2026-01-01.yaml",
            r"$id: wet_b
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '1'
    text: Eerste.
  - number: '2'
    text: Tweede.
",
        );
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["1".to_string()],
            2,
            &index,
            StopRules::default(),
        )
        .unwrap();
        assert_eq!(
            plan.gaps
                .iter()
                .filter(|g| g.kind == GapKind::ArticleNotInLaw)
                .map(|g| g.bwb_id.as_str())
                .collect::<Vec<_>>(),
            vec!["BWBR0000002"],
            "artikel 99 bestaat niet in wet_b: {:?}",
            plan.describe()
        );
        assert!(
            !plan.tasks.iter().any(|t| t.law_id == "wet_b"),
            "en levert geen taak op: {:?}",
            plan.describe()
        );
    }

    #[test]
    fn within_one_depth_a_producer_still_comes_before_its_reader() {
        let dir = tie_corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = plan_closure(
            dir.path(),
            "regulation/nl/wet/wortel/2026-01-01.yaml",
            &["1".to_string()],
            2,
            &index,
            StopRules::default(),
        )
        .unwrap();

        let position = |id: &str| {
            plan.tasks
                .iter()
                .position(|t| t.law_id == id)
                .unwrap_or_else(|| panic!("{id} ontbreekt in het plan: {:?}", plan.describe()))
        };
        let depth_of = |id: &str| plan.tasks.iter().find(|t| t.law_id == id).map(|t| t.depth);
        assert_eq!(depth_of("aaa_leest"), depth_of("mmm_levert"));
        assert!(
            position("mmm_levert") < position("aaa_leest"),
            "de producent hoort voor zijn lezer, ook op gelijke diepte: {:?}",
            plan.describe()
        );
    }

    #[test]
    fn a_shorter_route_found_later_does_not_cut_a_branch_inside_the_depth() {
        let dir = overtaking_corpus();
        let index = LawIndex::scan(dir.path()).unwrap();
        let plan = plan_closure(
            dir.path(),
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            &["1".to_string()],
            2,
            &index,
            StopRules::default(),
        )
        .unwrap();

        let depth_of = |id: &str| plan.tasks.iter().find(|t| t.law_id == id).map(|t| t.depth);
        assert_eq!(depth_of("wet_a"), Some(0));
        assert_eq!(depth_of("wet_b"), Some(1));
        assert_eq!(
            depth_of("wet_d"),
            Some(1),
            "wet_a artikel 4 haalt wet_d rechtstreeks"
        );
        assert_eq!(
            depth_of("wet_e"),
            Some(2),
            "wet_d staat op diepte 1, dus zijn kind past binnen diepte 2: {:?}",
            plan.describe()
        );
        assert!(
            !plan
                .gaps
                .iter()
                .any(|g| g.kind == GapKind::BeyondDepth && g.bwb_id == "BWBR0000005"),
            "een wet die in het plan staat is geen bekend gat: {:?}",
            plan.gaps
        );

        // Deepest first survives the fix: wet_e before wet_d before wet_a.
        let order: Vec<&str> = plan.tasks.iter().map(|t| t.law_id.as_str()).collect();
        let position = |id: &str| order.iter().position(|l| *l == id).unwrap();
        assert!(position("wet_e") < position("wet_d"));
        assert!(position("wet_d") < position("wet_a"));

        // The limit counts the plan the run will actually walk. On the broken
        // planner wet_e was missing, so five articles read as four and a limit
        // of four let the run through.
        assert_eq!(plan.articles(), 8, "{:?}", plan.describe());
        assert!(
            plan.refuse_above(7).is_some(),
            "de grens telt de artikelen die het plan nu wél heeft"
        );
    }

    #[test]
    fn a_plan_over_the_limit_refuses_with_its_own_size() {
        let plan = plan_of(&corpus(), 1);
        assert!(plan.refuse_above(100).is_none());
        let refusal = plan.refuse_above(1).unwrap();
        assert!(refusal.contains("articles"), "{refusal}");
    }

    /// Laws held in memory, as a server holds the facts of a traject.
    struct MemLaws {
        docs: BTreeMap<String, Value>,
        layers: BTreeMap<String, String>,
        implementors: BTreeMap<Node, Vec<Node>>,
    }

    impl MemLaws {
        fn new(laws: &[(&str, &str, &str)]) -> Self {
            let mut docs = BTreeMap::new();
            let mut layers = BTreeMap::new();
            for (bwb, layer, yaml) in laws {
                docs.insert((*bwb).to_string(), serde_yaml_ng::from_str(yaml).unwrap());
                layers.insert((*bwb).to_string(), (*layer).to_string());
            }
            Self {
                docs,
                layers,
                implementors: BTreeMap::new(),
            }
        }

        fn with_implementor(mut self, open_term: Node, implementor: Node) -> Self {
            self.implementors
                .entry(open_term)
                .or_default()
                .push(implementor);
            self
        }
    }

    impl LawGraphs for MemLaws {
        fn entry(&self, bwb_id: &str) -> Option<LawEntry> {
            let doc = self.docs.get(bwb_id)?;
            Some(LawEntry {
                bwb_id: bwb_id.to_string(),
                law_id: doc
                    .get("$id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                path: String::new(),
                layer: self.layers.get(bwb_id).cloned().unwrap_or_default(),
            })
        }

        fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw> {
            self.docs.get(bwb_id).map(loaded)
        }

        fn bwb_of(&self, law_id: &str) -> Option<String> {
            self.docs
                .iter()
                .find(|(_, d)| d.get("$id").and_then(Value::as_str) == Some(law_id))
                .map(|(bwb, _)| bwb.clone())
        }

        fn implementors(&self, bwb_id: &str, article: &str) -> Vec<Node> {
            self.implementors
                .get(&Node::new(bwb_id, article))
                .cloned()
                .unwrap_or_default()
        }
    }

    /// [`MemLaws`] that knows no `$id`s and no implementors: what the trait
    /// defaults leave a corpus that only has its laws by BWB id.
    struct ByBwbOnly(MemLaws);

    impl LawGraphs for ByBwbOnly {
        fn entry(&self, bwb_id: &str) -> Option<LawEntry> {
            self.0.entry(bwb_id)
        }

        fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw> {
            self.0.load(bwb_id)
        }
    }

    /// A reads a value of B through its model only (the text names no
    /// article), and B's producer reads a regeling through a binding.
    fn bound_laws() -> MemLaws {
        MemLaws::new(&[
            (
                "BWBA",
                "WET",
                r"
$id: wet_a
bwb_id: BWBA
articles:
  - number: '1'
    text: Het recht op toeslag.
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: wet_b
              output: toetsingsinkomen
",
            ),
            (
                "BWBB",
                "WET",
                r"
$id: wet_b
bwb_id: BWBB
articles:
  - number: '7'
    text: Niets.
  - number: '8'
    text: Het toetsingsinkomen.
    machine_readable:
      execution:
        input:
          - name: bedrag
            source:
              regulation: regeling_c
              output: bedrag
        output:
          - name: toetsingsinkomen
",
            ),
            (
                "BWBC",
                "MINISTERIELE_REGELING",
                r"
$id: regeling_c
bwb_id: BWBC
articles:
  - number: '2'
    text: Het bedrag.
    machine_readable:
      execution:
        output:
          - name: bedrag
",
            ),
        ])
    }

    fn bound_walk(follow_bindings: bool, node_cap: Option<usize>) -> Walk {
        Walk {
            depth: 3,
            rules: StopRules::default(),
            follow_bindings,
            node_cap,
        }
    }

    #[test]
    fn bindings_are_walked_only_when_asked_and_into_a_delegated_regulation() {
        let mut laws = bound_laws();
        let cited_only = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(false, None),
        )
        .unwrap();
        assert_eq!(cited_only.taken.len(), 1, "the text names no other law");

        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, None),
        )
        .unwrap();
        assert!(closure.taken["BWBB"].contains("8"));
        // A binding into a ministerial regulation is followed: the value read
        // there is part of the computation.
        assert!(closure.taken["BWBC"].contains("2"));
        assert_eq!(closure.law_depth["BWBC"], 2);
        let via = &closure.via[&Node::new("BWBC", "2")];
        assert_eq!(via.from, Node::new("BWBB", "8"));
        assert_eq!(via.kind, EdgeKind::Binds);
        assert!(!closure.via.contains_key(&Node::new("BWBA", "1")));
    }

    #[test]
    fn the_node_cap_stops_the_walk_and_says_so() {
        let mut laws = bound_laws();
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, Some(2)),
        )
        .unwrap();
        let taken: usize = closure.taken.values().map(BTreeSet::len).sum();
        assert_eq!(taken, 2);
        assert!(closure.node_capped);
    }

    #[test]
    fn a_binding_to_a_law_outside_the_corpus_is_a_known_gap() {
        let mut laws = MemLaws::new(&[(
            "BWBA",
            "WET",
            r"
$id: wet_a
bwb_id: BWBA
articles:
  - number: '1'
    text: Het recht.
    machine_readable:
      execution:
        input:
          - name: x
            source:
              regulation: onbekend
              output: x
",
        )]);
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, None),
        )
        .unwrap();
        assert_eq!(
            closure
                .gaps
                .get(&(GapKind::OutsideCorpus, "onbekend".to_string())),
            Some(&1)
        );
    }
    #[test]
    fn without_a_way_to_resolve_ids_a_binding_to_another_law_is_a_known_gap() {
        let mut laws = ByBwbOnly(bound_laws());
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, None),
        )
        .unwrap();
        assert_eq!(closure.taken.len(), 1, "{:?}", closure.taken);
        assert_eq!(
            closure.gaps,
            BTreeMap::from([((GapKind::OutsideCorpus, "wet_b".to_string()), 1)])
        );
    }

    #[test]
    fn a_binding_inside_the_law_is_followed_to_its_producer() {
        let mut laws = MemLaws::new(&[(
            "BWBA",
            "WET",
            r"
$id: wet_a
bwb_id: BWBA
articles:
  - number: '1'
    text: Het recht.
    machine_readable:
      execution:
        input:
          - name: drempel
            source:
              output: drempel
          - name: zelf
            source:
              output: recht
          - name: weg
            source:
              article: wet_a#99
        output:
          - name: recht
  - number: '2'
    text: De drempel.
    machine_readable:
      execution:
        output:
          - name: drempel
",
        )]);
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, None),
        )
        .unwrap();
        assert_eq!(
            closure.taken["BWBA"],
            BTreeSet::from(["1".to_string(), "2".to_string()])
        );
        let via = &closure.via[&Node::new("BWBA", "2")];
        assert_eq!(via.from, Node::new("BWBA", "1"));
        assert_eq!(via.kind, EdgeKind::Binds);
        // The article the model calls does not exist; reading its own output
        // is no edge at all.
        assert_eq!(
            closure.gaps,
            BTreeMap::from([((GapKind::ArticleNotInLaw, "BWBA".to_string()), 1)])
        );
    }

    #[test]
    fn an_implementor_is_followed_inside_the_law_and_out_of_it() {
        let mut laws = MemLaws::new(&[
            (
                "BWBA",
                "WET",
                r"
$id: wet_a
bwb_id: BWBA
articles:
  - number: '1'
    text: De premie wordt nader bepaald.
  - number: '2'
    text: De premie in een bijzonder geval.
",
            ),
            (
                "BWBR",
                "MINISTERIELE_REGELING",
                r"
$id: regeling_r
bwb_id: BWBR
articles:
  - number: '7'
    text: De premie bedraagt honderd euro.
",
            ),
        ])
        .with_implementor(Node::new("BWBA", "1"), Node::new("BWBA", "2"))
        .with_implementor(Node::new("BWBA", "1"), Node::new("BWBR", "7"));
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(true, None),
        )
        .unwrap();
        assert_eq!(
            closure.taken["BWBA"],
            BTreeSet::from(["1".to_string(), "2".to_string()])
        );
        assert_eq!(closure.taken["BWBR"], BTreeSet::from(["7".to_string()]));
        assert_eq!(closure.law_depth["BWBR"], 1);
        // Only the step out of the law is a read between laws.
        assert_eq!(
            closure.reads,
            BTreeMap::from([("BWBA".to_string(), BTreeSet::from(["BWBR".to_string()]))])
        );
        for node in [Node::new("BWBA", "2"), Node::new("BWBR", "7")] {
            assert_eq!(closure.via[&node].kind, EdgeKind::Implements, "{node:?}");
        }
    }

    #[test]
    fn a_shorter_route_found_later_lowers_a_laws_depth() {
        // A/1 reaches B; B/5 cites a new article of A and C/1, so C is first
        // met at depth 2. A/9 then cites C/2 from depth 0: C sits at 1.
        let mut laws = MemLaws::new(&[
            (
                "BWBA",
                "WET",
                r"
$id: wet_a
bwb_id: BWBA
articles:
  - number: '1'
    text: Zie wet B.
    references:
      - id: r1
        bwb_id: BWBB
        artikel: '5'
  - number: '9'
    text: Zie wet C.
    references:
      - id: r1
        bwb_id: BWBC
        artikel: '2'
",
            ),
            (
                "BWBB",
                "WET",
                r"
$id: wet_b
bwb_id: BWBB
articles:
  - number: '5'
    text: Zie wet A en wet C.
    references:
      - id: r1
        bwb_id: BWBA
        artikel: '9'
      - id: r2
        bwb_id: BWBC
        artikel: '1'
",
            ),
            (
                "BWBC",
                "WET",
                r"
$id: wet_c
bwb_id: BWBC
articles:
  - number: '1'
    text: Het eerste.
  - number: '2'
    text: Het tweede.
",
            ),
        ]);
        let closure = walk_closure(
            &mut laws,
            "BWBA",
            &["1".to_string()],
            bound_walk(false, None),
        )
        .unwrap();
        assert_eq!(
            closure.taken["BWBC"],
            BTreeSet::from(["1".to_string(), "2".to_string()])
        );
        assert_eq!(closure.law_depth["BWBA"], 0);
        assert_eq!(closure.law_depth["BWBB"], 1);
        assert_eq!(closure.law_depth["BWBC"], 1, "{:?}", closure.via);
    }
}
