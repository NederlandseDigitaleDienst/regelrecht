//! The chain of an article and the hotspots of a traject, from the facts of
//! its laws.
//!
//! **The chain** answers "what does enriching this article touch?": every
//! article, in this law and in others in the traject, that it reaches by what
//! its text cites, what its model reads, and who fills its open terms. It is
//! the closure walk of the enrich pipeline
//! ([`regelrecht_pipeline::enrich_v2::closure::walk_closure`]) over the facts
//! the traject corpus keeps, with bindings followed: depth counts law jumps,
//! a step inside a law is free, a begripsbepaling is a leaf, and a citation
//! into a delegated regulation stops there while a binding into one does not.
//!
//! **A hotspot** is an article where enrichment helps most: it has a gap (no
//! model, a broken binding, an open term nobody fills) and other articles
//! depend on it. The score is a sum of named signals, so the reason can be
//! shown next to the number. The table is in the editor-api docs page.
//!
//! Pure: no I/O, no clock. The caller passes the facts and how many laws it
//! could not read.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use regelrecht_pipeline::enrich_v2::closure::{
    EdgeKind, GapKind, LawEntry, LawGraphs, LoadedLaw, StopRules, Walk, walk_closure,
};
use regelrecht_pipeline::enrich_v2::facts::{ArticleFacts, LawFacts};
use regelrecht_pipeline::enrich_v2::refgraph::{Binding, Graph, Node, top_article};
use serde::Serialize;

/// Law jumps the chain walks when the request names none.
pub const DEFAULT_DEPTH: usize = 3;
/// The most law jumps a request may ask for ("alles" in the editor).
pub const MAX_DEPTH: usize = 6;
/// The most articles one chain takes; beyond it the chain says it is capped.
pub const NODE_CAP: usize = 2000;

/// Signal M: the article says something and has no model.
pub const POINTS_MISSING_MR: u32 = 3;
/// Signal D: per dangling binding in the model.
pub const POINTS_PER_DANGLING: u32 = 2;
/// Signal D, at most.
pub const MAX_DANGLING_POINTS: u32 = 6;
/// Signal U: per binding to another law that the traject cannot resolve.
pub const POINTS_PER_UNRESOLVED: u32 = 2;
/// Signal U, at most.
pub const MAX_UNRESOLVED_POINTS: u32 = 6;
/// Signal O: per open term nothing in the traject fills.
pub const POINTS_PER_OPEN_TERM: u32 = 1;
/// Signal O, at most.
pub const MAX_OPEN_TERM_POINTS: u32 = 3;
/// Signal B: per article elsewhere whose model reads this one.
pub const POINTS_PER_BINDER: u32 = 2;
/// Signal B, at most.
pub const MAX_BINDER_POINTS: u32 = 10;
/// Signal R: per article whose text cites this one.
pub const POINTS_PER_CITER: u32 = 1;
/// Signal R, at most.
pub const MAX_CITER_POINTS: u32 = 5;
/// Signal K: on the chain of the article the engineer chose.
pub const POINTS_ON_CHAIN: u32 = 3;

/// How many references a reason lists by name.
const MAX_REASON_REFS: usize = 5;

/// `"<law>#<article>"`, the key the editor uses for an article.
fn article_key(law_id: &str, article: &str) -> String {
    format!("{law_id}#{article}")
}

/// Why the chain could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// The traject has no law with this id, or could not read it.
    UnknownLaw,
    /// The law has no such article.
    UnknownArticle,
}

/// One article of the chain or of the traject, by law and number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleRef {
    pub law_id: String,
    pub article: String,
}

/// How the chain reached an article.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViaRef {
    pub law_id: String,
    pub article: String,
    pub kind: EdgeKind,
}

/// What is missing or broken in one article.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NodeGaps {
    pub missing_mr: bool,
    pub dangling: usize,
    pub unresolved: usize,
    pub open_terms: usize,
}

/// One signal behind a hotspot score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reason {
    /// `M`, `D`, `U`, `O`, `B`, `R` or `K` (see the constants above).
    pub signal: &'static str,
    pub points: u32,
    /// The reason in a few Dutch words, without the points.
    pub detail: String,
    /// The articles it is about, `"<law>#<article>"`, at most a handful.
    pub refs: Vec<String>,
}

/// A score with the signals that make it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hotspot {
    pub score: u32,
    pub reasons: Vec<Reason>,
}

/// One article on the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainNode {
    pub law_id: String,
    pub bwb_id: String,
    pub article: String,
    /// Law jumps from the root's law.
    pub depth: usize,
    pub has_mr: bool,
    pub gaps: NodeGaps,
    /// `None` for the root.
    pub via: Option<ViaRef>,
    pub hotspot: Option<Hotspot>,
}

/// An edge the chain recorded instead of following.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainGap {
    /// `outside_corpus`, `delegated`, `no_article`, `article_not_in_law` or
    /// `beyond_depth`.
    pub kind: &'static str,
    /// BWB id of the law the edge points at, or the `$id` a binding names.
    pub bwb_id: String,
    pub occurrences: usize,
}

/// What the chain could not cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Partial {
    /// Laws of the traject whose body could not be read.
    pub unread_laws: usize,
    /// Some law lay beyond the depth asked for.
    pub depth_capped: bool,
    /// The walk stopped at [`NODE_CAP`] articles.
    pub node_capped: bool,
}

/// `GET …/articles/{article}/chain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chain {
    pub root: ArticleRef,
    /// Root first, then by depth, law and document order.
    pub nodes: Vec<ChainNode>,
    pub gaps: Vec<ChainGap>,
    pub partial: Partial,
}

/// One hotspot in the traject list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrajectHotspot {
    pub law_id: String,
    pub bwb_id: String,
    pub article: String,
    pub score: u32,
    pub reasons: Vec<Reason>,
    /// Articles elsewhere that bind to or cite it.
    pub dependants: usize,
}

/// Score and dependants of one hotspot article, for the article list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleScore {
    pub score: u32,
    pub count: usize,
}

/// `GET /api/trajects/{ref}/hotspots`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hotspots {
    pub hotspots: Vec<TrajectHotspot>,
    /// Every hotspot of the traject (or of the law asked for), by
    /// `"<law>#<article>"`.
    pub per_article: BTreeMap<String, ArticleScore>,
    /// Laws whose body could not be read: a missing hotspot is no proof.
    pub partial: bool,
}

/// The laws of a traject with the indexes both questions need.
pub struct Stelsel<'a> {
    laws: &'a BTreeMap<String, Arc<LawFacts>>,
    /// The walk's key per law: its BWB id, or a stand-in for a law without
    /// one (a regelwerk made from a document).
    key_of: BTreeMap<&'a str, String>,
    law_of_key: BTreeMap<String, &'a str>,
    /// Articles whose model reads `(law, article)`.
    binders: BTreeMap<(String, String), BTreeSet<(String, String)>>,
    /// Articles whose text cites `(law, article)`.
    citers: BTreeMap<(String, String), BTreeSet<(String, String)>>,
    /// Bindings of `(law, article)` the traject cannot resolve.
    unresolved: BTreeMap<(String, String), usize>,
    /// `(law, open term)` pairs something in the traject fills.
    filled: BTreeSet<(String, String)>,
    /// Who fills an open term of `(law, article)`, as walk nodes.
    implementors: BTreeMap<(String, String), Vec<Node>>,
}

impl<'a> Stelsel<'a> {
    /// Index the facts of a traject's laws, keyed by law id.
    #[must_use]
    pub fn new(laws: &'a BTreeMap<String, Arc<LawFacts>>) -> Self {
        let mut stelsel = Self {
            laws,
            key_of: BTreeMap::new(),
            law_of_key: BTreeMap::new(),
            binders: BTreeMap::new(),
            citers: BTreeMap::new(),
            unresolved: BTreeMap::new(),
            filled: BTreeSet::new(),
            implementors: BTreeMap::new(),
        };
        for (law_id, facts) in laws {
            // A second law with the same BWB id (a copy made in the traject)
            // gets the stand-in key too, or the walk would load the first.
            let key = if facts.bwb_id.is_empty() || stelsel.law_of_key.contains_key(&facts.bwb_id) {
                format!("$id:{law_id}")
            } else {
                facts.bwb_id.clone()
            };
            stelsel.law_of_key.insert(key.clone(), law_id);
            stelsel.key_of.insert(law_id, key);
        }
        for (law_id, facts) in laws {
            for article in &facts.articles {
                stelsel.index_article(law_id, facts, article);
            }
        }
        stelsel
    }

    fn index_article(&mut self, law_id: &str, facts: &LawFacts, article: &ArticleFacts) {
        let here = (law_id.to_string(), article.number.clone());
        for binding in &article.binds {
            match self.resolve(law_id, facts, binding) {
                Resolved::To(target) => {
                    if target != here {
                        self.binders.entry(target).or_default().insert(here.clone());
                    }
                }
                Resolved::Unresolved => {
                    *self.unresolved.entry(here.clone()).or_default() += 1;
                }
                Resolved::Ignored => {}
            }
        }
        for cited in &article.cites {
            let Some(target_law) = self.law_by_bwb(&cited.bwb_id, law_id, facts) else {
                continue;
            };
            let target = (target_law, cited.article.clone());
            if target != here {
                self.citers.entry(target).or_default().insert(here.clone());
            }
        }
        for implements in &article.implements {
            self.filled
                .insert((implements.law_id.clone(), implements.open_term.clone()));
            let Some(target_facts) = self.laws.get(&implements.law_id) else {
                continue;
            };
            let target_article = implements
                .article
                .as_deref()
                .map(|a| top_article(a).to_string())
                .or_else(|| {
                    target_facts
                        .articles
                        .iter()
                        .find(|a| a.open_terms.contains(&implements.open_term))
                        .map(|a| a.number.clone())
                });
            let (Some(target_article), Some(own_key), Some(target_key)) = (
                target_article,
                self.key_of.get(law_id).cloned(),
                self.key_of.get(implements.law_id.as_str()).cloned(),
            ) else {
                continue;
            };
            self.implementors
                .entry((target_key, target_article))
                .or_default()
                .push(Node::new(own_key, article.number.clone()));
        }
    }

    /// The law a citation points at: the own law for the own BWB id (or an
    /// empty one), else the law with that BWB id in the traject.
    fn law_by_bwb(&self, bwb_id: &str, law_id: &str, facts: &LawFacts) -> Option<String> {
        if bwb_id.is_empty() || bwb_id == facts.bwb_id {
            return Some(law_id.to_string());
        }
        self.law_of_key.get(bwb_id).map(|l| (*l).to_string())
    }

    /// Where a binding of `law_id` lands.
    fn resolve(&self, law_id: &str, facts: &LawFacts, binding: &Binding) -> Resolved {
        let (target_law, target_facts, cross_law) = match &binding.regulation {
            None => (law_id, facts, false),
            Some(other) => match self.laws.get(other) {
                Some(f) => (other.as_str(), f.as_ref(), true),
                None => return Resolved::Unresolved,
            },
        };
        let article = match (&binding.article, &binding.output) {
            (Some(article), _) => {
                let top = top_article(article);
                target_facts.article(top).map(|_| top.to_string())
            }
            (None, Some(output)) => target_facts
                .graph
                .producers
                .get(output)
                .and_then(|p| p.first())
                .map(|p| top_article(&p.entry).to_string()),
            (None, None) => return Resolved::Ignored,
        };
        match article {
            Some(article) => Resolved::To((target_law.to_string(), article)),
            // Inside one law an unproduced output is a dangling binding,
            // which signal D already counts.
            None if cross_law => Resolved::Unresolved,
            None => Resolved::Ignored,
        }
    }

    fn gaps_of(&self, law_id: &str, article: &ArticleFacts) -> NodeGaps {
        let here = (law_id.to_string(), article.number.clone());
        NodeGaps {
            missing_mr: article.missing_mr,
            dangling: article.dangling,
            unresolved: self.unresolved.get(&here).copied().unwrap_or(0),
            open_terms: article
                .open_terms
                .iter()
                .filter(|term| !self.filled.contains(&(law_id.to_string(), (*term).clone())))
                .count(),
        }
    }

    /// The hotspot score of one article, or `None` when it has no gap.
    fn score(&self, law_id: &str, article: &ArticleFacts, on_chain: bool) -> Option<Hotspot> {
        let gaps = self.gaps_of(law_id, article);
        let here = (law_id.to_string(), article.number.clone());
        let mut reasons: Vec<Reason> = Vec::new();
        if gaps.missing_mr {
            reasons.push(reason(
                "M",
                POINTS_MISSING_MR,
                "Geen machine_readable".into(),
                &[],
            ));
        }
        if gaps.dangling > 0 {
            reasons.push(reason(
                "D",
                capped(gaps.dangling, POINTS_PER_DANGLING, MAX_DANGLING_POINTS),
                count_phrase(gaps.dangling, "losse binding", "losse bindingen"),
                &[],
            ));
        }
        if gaps.unresolved > 0 {
            reasons.push(reason(
                "U",
                capped(
                    gaps.unresolved,
                    POINTS_PER_UNRESOLVED,
                    MAX_UNRESOLVED_POINTS,
                ),
                count_phrase(
                    gaps.unresolved,
                    "binding naar een andere wet die niet aankomt",
                    "bindingen naar een andere wet die niet aankomen",
                ),
                &[],
            ));
        }
        if gaps.open_terms > 0 {
            reasons.push(reason(
                "O",
                capped(gaps.open_terms, POINTS_PER_OPEN_TERM, MAX_OPEN_TERM_POINTS),
                count_phrase(
                    gaps.open_terms,
                    "open term die niemand invult",
                    "open termen die niemand invult",
                ),
                &[],
            ));
        }
        // No gap, no hotspot: an article that is complete is not where
        // enrichment helps, however much depends on it.
        if reasons.is_empty() {
            return None;
        }
        if let Some(binders) = self.binders.get(&here).filter(|b| !b.is_empty()) {
            reasons.push(reason(
                "B",
                capped(binders.len(), POINTS_PER_BINDER, MAX_BINDER_POINTS),
                count_phrase(
                    binders.len(),
                    "artikel elders leest dit",
                    "artikelen elders lezen dit",
                ),
                &refs(binders),
            ));
        }
        if let Some(citers) = self.citers.get(&here).filter(|c| !c.is_empty()) {
            reasons.push(reason(
                "R",
                capped(citers.len(), POINTS_PER_CITER, MAX_CITER_POINTS),
                count_phrase(
                    citers.len(),
                    "artikel verwijst hiernaar",
                    "artikelen verwijzen hiernaar",
                ),
                &refs(citers),
            ));
        }
        if on_chain {
            reasons.push(reason(
                "K",
                POINTS_ON_CHAIN,
                "Ligt op de keten van het gekozen artikel".into(),
                &[],
            ));
        }
        Some(Hotspot {
            score: reasons.iter().map(|r| r.points).sum(),
            reasons,
        })
    }

    /// Articles elsewhere that bind to or cite `(law_id, article)`.
    fn dependants(&self, law_id: &str, article: &str) -> usize {
        let here = (law_id.to_string(), article.to_string());
        let mut all: BTreeSet<&(String, String)> = BTreeSet::new();
        all.extend(self.binders.get(&here).into_iter().flatten());
        all.extend(self.citers.get(&here).into_iter().flatten());
        all.len()
    }

    /// The chain of `article` in `law_id`, `depth` law jumps deep (clamped
    /// to [`MAX_DEPTH`]).
    ///
    /// # Errors
    ///
    /// [`ChainError`] when the law or the article is not in the traject.
    /// `$id`s of laws that articles on `chain` bind to and that `laws` does
    /// not hold yet: what a lazy walk reads next.
    #[must_use]
    pub fn unread_bound_laws(&self, chain: &Chain) -> Vec<String> {
        let mut out = BTreeSet::new();
        for node in &chain.nodes {
            let Some(facts) = self.laws.get(node.law_id.as_str()) else {
                continue;
            };
            let Some(article) = facts.article(&node.article) else {
                continue;
            };
            for binding in &article.binds {
                if let Some(target) = &binding.regulation
                    && !self.laws.contains_key(target.as_str())
                {
                    out.insert(target.clone());
                }
            }
        }
        out.into_iter().collect()
    }

    pub fn chain(
        &self,
        law_id: &str,
        article: &str,
        depth: usize,
        unread_laws: usize,
    ) -> Result<Chain, ChainError> {
        let root_key = self.key_of.get(law_id).ok_or(ChainError::UnknownLaw)?;
        let root_facts = self.laws.get(law_id).ok_or(ChainError::UnknownLaw)?;
        let article = top_article(article).to_string();
        if root_facts.article(&article).is_none() {
            return Err(ChainError::UnknownArticle);
        }
        let walk = Walk {
            depth: depth.min(MAX_DEPTH),
            rules: StopRules::default(),
            follow_bindings: true,
            node_cap: Some(NODE_CAP),
        };
        let mut graphs = FactsLaws { stelsel: self };
        let closure = walk_closure(&mut graphs, root_key, std::slice::from_ref(&article), walk)
            .map_err(|_| ChainError::UnknownArticle)?;

        let mut nodes: Vec<(usize, usize, ChainNode)> = Vec::new();
        for (key, articles) in &closure.taken {
            let Some(node_law) = self.law_of_key.get(key) else {
                continue;
            };
            let Some(facts) = self.laws.get(*node_law) else {
                continue;
            };
            let depth = closure.law_depth.get(key).copied().unwrap_or(0);
            for (position, facts_article) in facts.articles.iter().enumerate() {
                if !articles.contains(&facts_article.number) {
                    continue;
                }
                let via = closure
                    .via
                    .get(&Node::new(key.clone(), facts_article.number.clone()))
                    .and_then(|v| {
                        self.law_of_key.get(&v.from.bwb_id).map(|from_law| ViaRef {
                            law_id: (*from_law).to_string(),
                            article: v.from.article.clone(),
                            kind: v.kind,
                        })
                    });
                let is_root = *node_law == law_id && facts_article.number == article;
                nodes.push((
                    depth,
                    position,
                    ChainNode {
                        law_id: (*node_law).to_string(),
                        bwb_id: facts.bwb_id.clone(),
                        article: facts_article.number.clone(),
                        depth,
                        has_mr: facts_article.has_mr,
                        gaps: self.gaps_of(node_law, facts_article),
                        via: if is_root { None } else { via },
                        hotspot: self.score(node_law, facts_article, true),
                    },
                ));
            }
        }
        nodes.sort_by(|(da, pa, a), (db, pb, b)| {
            let root_a = a.law_id == law_id && a.article == article;
            let root_b = b.law_id == law_id && b.article == article;
            root_b
                .cmp(&root_a)
                .then(da.cmp(db))
                .then(a.law_id.cmp(&b.law_id))
                .then(pa.cmp(pb))
        });
        let depth_capped = closure
            .gaps
            .keys()
            .any(|(kind, _)| *kind == GapKind::BeyondDepth);
        Ok(Chain {
            root: ArticleRef {
                law_id: law_id.to_string(),
                article,
            },
            nodes: nodes.into_iter().map(|(_, _, n)| n).collect(),
            gaps: closure
                .gaps
                .into_iter()
                .map(|((kind, bwb_id), occurrences)| ChainGap {
                    kind: gap_kind_name(&kind),
                    bwb_id,
                    occurrences,
                })
                .collect(),
            partial: Partial {
                unread_laws,
                depth_capped,
                node_capped: closure.node_capped,
            },
        })
    }

    /// The traject's hotspots, highest first, `limit` of them; with
    /// `law_id`, only those of that law.
    #[must_use]
    pub fn hotspots(&self, limit: usize, law_id: Option<&str>, unread_laws: usize) -> Hotspots {
        let mut all: Vec<(u32, &str, usize, TrajectHotspot)> = Vec::new();
        for (id, facts) in self.laws {
            if law_id.is_some_and(|wanted| wanted != id) {
                continue;
            }
            for (position, article) in facts.articles.iter().enumerate() {
                let Some(hotspot) = self.score(id, article, false) else {
                    continue;
                };
                all.push((
                    hotspot.score,
                    id.as_str(),
                    position,
                    TrajectHotspot {
                        law_id: id.clone(),
                        bwb_id: facts.bwb_id.clone(),
                        article: article.number.clone(),
                        score: hotspot.score,
                        reasons: hotspot.reasons,
                        dependants: self.dependants(id, &article.number),
                    },
                ));
            }
        }
        // Highest first; a tie goes to the law and then to document order,
        // so the list is stable between requests.
        all.sort_by(|(sa, la, pa, _), (sb, lb, pb, _)| {
            sb.cmp(sa).then(la.cmp(lb)).then(pa.cmp(pb))
        });
        let per_article = all
            .iter()
            .map(|(_, _, _, h)| {
                (
                    article_key(&h.law_id, &h.article),
                    ArticleScore {
                        score: h.score,
                        count: h.dependants,
                    },
                )
            })
            .collect();
        Hotspots {
            hotspots: all.into_iter().take(limit).map(|(_, _, _, h)| h).collect(),
            per_article,
            partial: unread_laws > 0,
        }
    }
}

/// Where a binding lands, as far as the traject can tell.
enum Resolved {
    To((String, String)),
    Unresolved,
    Ignored,
}

/// The facts as the closure walk reads them.
struct FactsLaws<'s, 'a> {
    stelsel: &'s Stelsel<'a>,
}

impl LawGraphs for FactsLaws<'_, '_> {
    fn entry(&self, bwb_id: &str) -> Option<LawEntry> {
        let law_id = self.stelsel.law_of_key.get(bwb_id)?;
        let facts = self.stelsel.laws.get(*law_id)?;
        Some(LawEntry {
            bwb_id: bwb_id.to_string(),
            law_id: (*law_id).to_string(),
            path: String::new(),
            layer: facts.layer.clone(),
        })
    }

    fn load(&mut self, bwb_id: &str) -> Option<LoadedLaw> {
        let law_id = self.stelsel.law_of_key.get(bwb_id)?;
        let facts = self.stelsel.laws.get(*law_id)?;
        let mut law = facts.loaded();
        if facts.bwb_id != bwb_id {
            law.graph = rekeyed(law.graph, bwb_id);
        }
        Some(law)
    }

    fn bwb_of(&self, law_id: &str) -> Option<String> {
        self.stelsel.key_of.get(law_id).cloned()
    }

    fn implementors(&self, bwb_id: &str, article: &str) -> Vec<Node> {
        self.stelsel
            .implementors
            .get(&(bwb_id.to_string(), article.to_string()))
            .cloned()
            .unwrap_or_default()
    }
}

/// A graph scanned from a law without a BWB id names its own nodes with an
/// empty id; give them the walk's stand-in key, or every step inside the law
/// would read as a step to an unknown one.
fn rekeyed(mut graph: Graph, key: &str) -> Graph {
    let own = std::mem::take(&mut graph.bwb_id);
    let rename = |node: &Node| {
        if node.bwb_id == own {
            Node::new(key, node.article.clone())
        } else {
            node.clone()
        }
    };
    graph.depends_on = graph
        .depends_on
        .iter()
        .map(|(from, targets)| (rename(from), targets.iter().map(rename).collect()))
        .collect();
    graph.bwb_id = key.to_string();
    graph
}

fn gap_kind_name(kind: &GapKind) -> &'static str {
    match kind {
        GapKind::OutsideCorpus => "outside_corpus",
        GapKind::Delegated => "delegated",
        GapKind::NoArticle => "no_article",
        GapKind::ArticleNotInLaw => "article_not_in_law",
        GapKind::BeyondDepth => "beyond_depth",
    }
}

fn reason(signal: &'static str, points: u32, detail: String, refs: &[String]) -> Reason {
    Reason {
        signal,
        points,
        detail,
        refs: refs.to_vec(),
    }
}

/// `count × per`, at most `max`.
fn capped(count: usize, per: u32, max: u32) -> u32 {
    u32::try_from(count)
        .unwrap_or(u32::MAX)
        .saturating_mul(per)
        .min(max)
}

fn count_phrase(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn refs(articles: &BTreeSet<(String, String)>) -> Vec<String> {
    articles
        .iter()
        .take(MAX_REASON_REFS)
        .map(|(law, article)| article_key(law, article))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn laws(bodies: &[&str]) -> BTreeMap<String, Arc<LawFacts>> {
        bodies
            .iter()
            .map(|b| {
                let facts = LawFacts::from_yaml(b).unwrap();
                (facts.law_id.clone(), Arc::new(facts))
            })
            .collect()
    }

    const ZORGTOESLAG: &str = r"
$id: zorgtoeslag
bwb_id: BWBZ
regulatory_layer: WET
articles:
  - number: '1'
    text: In deze wet wordt verstaan onder verzekerde de persoon.
  - number: '2'
    text: De verzekerde, bedoeld in artikel 1, heeft aanspraak.
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: awir
              output: toetsingsinkomen
          - name: premie
            source:
              regulation: regeling
              output: standaardpremie
        output:
          - name: heeft_recht
  - number: '4'
    text: De standaardpremie wordt bij regeling bepaald.
    machine_readable:
      open_terms:
        - id: standaardpremie
";

    const AWIR: &str = r"
$id: awir
bwb_id: BWBA
regulatory_layer: WET
articles:
  - number: '8'
    text: Het toetsingsinkomen is het verzamelinkomen.
  - number: '9'
    text: Iets anders.
";

    const REGELING: &str = r"
$id: regeling
bwb_id: BWBR
regulatory_layer: MINISTERIELE_REGELING
articles:
  - number: '1'
    text: De standaardpremie bedraagt.
    machine_readable:
      implements:
        - law: zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
";

    #[test]
    fn the_chain_follows_bindings_and_names_its_path() {
        let facts = laws(&[ZORGTOESLAG, AWIR, REGELING]);
        let stelsel = Stelsel::new(&facts);
        let chain = stelsel.chain("zorgtoeslag", "2", 3, 0).unwrap();
        assert_eq!(chain.nodes[0].law_id, "zorgtoeslag");
        assert_eq!(chain.nodes[0].article, "2");
        assert!(chain.nodes[0].via.is_none());

        let keys: Vec<String> = chain
            .nodes
            .iter()
            .map(|n| article_key(&n.law_id, &n.article))
            .collect();
        // The definition is cited and taken (a leaf); the regeling is read
        // through a binding although it is a delegated layer.
        assert!(keys.contains(&"zorgtoeslag#1".to_string()), "{keys:?}");
        assert!(keys.contains(&"regeling#1".to_string()), "{keys:?}");
        // awir 8 is read through a binding, but awir has no producer of
        // toetsingsinkomen: the binding does not land, so the chain does not
        // reach it and article 2 counts it as unresolved.
        assert!(!keys.contains(&"awir#8".to_string()), "{keys:?}");
        assert_eq!(chain.nodes[0].gaps.unresolved, 1);

        let regeling = chain.nodes.iter().find(|n| n.law_id == "regeling").unwrap();
        assert_eq!(regeling.depth, 1);
        let via = regeling.via.as_ref().unwrap();
        assert_eq!(
            (via.law_id.as_str(), via.article.as_str()),
            ("zorgtoeslag", "2")
        );
        assert_eq!(via.kind, EdgeKind::Binds);
        assert!(!chain.partial.node_capped);
    }

    #[test]
    fn an_unknown_law_or_article_is_refused() {
        let facts = laws(&[ZORGTOESLAG]);
        let stelsel = Stelsel::new(&facts);
        assert_eq!(
            stelsel.chain("bestaat_niet", "1", 3, 0),
            Err(ChainError::UnknownLaw)
        );
        assert_eq!(
            stelsel.chain("zorgtoeslag", "99", 3, 0),
            Err(ChainError::UnknownArticle)
        );
    }

    #[test]
    fn a_filled_open_term_is_no_gap_and_an_empty_one_is() {
        let with_regeling = laws(&[ZORGTOESLAG, REGELING]);
        let stelsel = Stelsel::new(&with_regeling);
        let four = with_regeling["zorgtoeslag"].article("4").unwrap();
        assert_eq!(stelsel.gaps_of("zorgtoeslag", four).open_terms, 0);

        let alone = laws(&[ZORGTOESLAG]);
        let stelsel = Stelsel::new(&alone);
        let four = alone["zorgtoeslag"].article("4").unwrap();
        assert_eq!(stelsel.gaps_of("zorgtoeslag", four).open_terms, 1);
    }

    #[test]
    fn a_hotspot_needs_a_gap_and_scores_what_depends_on_it() {
        let facts = laws(&[ZORGTOESLAG, AWIR, REGELING]);
        let stelsel = Stelsel::new(&facts);
        // Article 1 has no model (M, +3) and article 2 cites it (R, +1).
        let one = facts["zorgtoeslag"].article("1").unwrap();
        let hotspot = stelsel.score("zorgtoeslag", one, false).unwrap();
        assert_eq!(hotspot.score, POINTS_MISSING_MR + POINTS_PER_CITER);
        let signals: Vec<&str> = hotspot.reasons.iter().map(|r| r.signal).collect();
        assert_eq!(signals, ["M", "R"]);
        assert_eq!(hotspot.reasons[1].refs, ["zorgtoeslag#2"]);

        // The regeling's article is complete: read by article 2, but no gap,
        // so no hotspot.
        let regeling = facts["regeling"].article("1").unwrap();
        assert!(stelsel.score("regeling", regeling, false).is_none());
        // On the chain it gets K on top of its own score.
        let on_chain = stelsel.score("zorgtoeslag", one, true).unwrap();
        assert_eq!(on_chain.score, hotspot.score + POINTS_ON_CHAIN);
    }

    #[test]
    fn signals_are_capped() {
        assert_eq!(capped(2, POINTS_PER_DANGLING, MAX_DANGLING_POINTS), 4);
        assert_eq!(
            capped(9, POINTS_PER_DANGLING, MAX_DANGLING_POINTS),
            MAX_DANGLING_POINTS
        );
        assert_eq!(
            capped(50, POINTS_PER_BINDER, MAX_BINDER_POINTS),
            MAX_BINDER_POINTS
        );
    }

    #[test]
    fn hotspots_are_ranked_and_filtered_per_law() {
        let facts = laws(&[ZORGTOESLAG, AWIR, REGELING]);
        let stelsel = Stelsel::new(&facts);
        let all = stelsel.hotspots(10, None, 0);
        assert!(!all.partial);
        let scores: Vec<u32> = all.hotspots.iter().map(|h| h.score).collect();
        let mut sorted = scores.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(scores, sorted);
        assert!(all.per_article.contains_key("zorgtoeslag#1"));
        assert_eq!(all.per_article["zorgtoeslag#1"].count, 1);

        let awir = stelsel.hotspots(10, Some("awir"), 2);
        assert!(awir.partial);
        assert!(awir.hotspots.iter().all(|h| h.law_id == "awir"));
        assert_eq!(stelsel.hotspots(1, None, 0).hotspots.len(), 1);
    }

    #[test]
    fn a_lazy_walk_asks_for_the_laws_its_articles_bind_to() {
        let start = r"
$id: regeling
bwb_id: BWBR0000010
articles:
  - number: '1'
    text: Het bedrag.
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: wet_inkomen
              output: toetsingsinkomen
  - number: '2'
    text: Losstaand.
    machine_readable:
      execution:
        input:
          - name: x
            source:
              regulation: wet_elders
              output: y
";
        let only_start = laws(&[start]);
        let stelsel = Stelsel::new(&only_start);
        let chain = stelsel.chain("regeling", "1", 3, 0).unwrap();
        // Article 2 is not on the chain of article 1, so its law is not asked for.
        assert_eq!(stelsel.unread_bound_laws(&chain), ["wet_inkomen"]);

        let with_target = laws(&[
            start,
            r"
$id: wet_inkomen
bwb_id: BWBR0000011
articles:
  - number: '3'
    text: Het toetsingsinkomen.
    machine_readable:
      execution:
        output:
          - name: toetsingsinkomen
",
        ]);
        let stelsel = Stelsel::new(&with_target);
        let chain = stelsel.chain("regeling", "1", 3, 0).unwrap();
        assert!(stelsel.unread_bound_laws(&chain).is_empty());
        assert!(
            chain
                .nodes
                .iter()
                .any(|n| n.law_id == "wet_inkomen" && n.article == "3"),
            "{:?}",
            chain.nodes
        );
    }

    #[test]
    fn a_law_without_a_bwb_id_still_walks_inside_itself() {
        let facts = laws(&[r"
$id: eigen_regelwerk
articles:
  - number: '1'
    text: Zie artikel 2.
  - number: '2'
    text: Het bedrag.
"]);
        let stelsel = Stelsel::new(&facts);
        let chain = stelsel.chain("eigen_regelwerk", "1", 3, 0).unwrap();
        let articles: Vec<&str> = chain.nodes.iter().map(|n| n.article.as_str()).collect();
        assert_eq!(articles, ["1", "2"]);
        assert!(chain.gaps.is_empty(), "{:?}", chain.gaps);
    }

    #[test]
    fn a_copy_with_the_same_bwb_id_walks_its_own_articles() {
        let copy = ZORGTOESLAG.replace("$id: zorgtoeslag", "$id: zorgtoeslag_kopie");
        let copy = copy.replace(
            "  - number: '4'",
            "  - number: '5'\n    text: Alleen in de kopie.\n  - number: '4'",
        );
        let facts = laws(&[ZORGTOESLAG, &copy]);
        let stelsel = Stelsel::new(&facts);
        let chain = stelsel.chain("zorgtoeslag_kopie", "5", 3, 0).unwrap();
        assert_eq!(chain.nodes[0].law_id, "zorgtoeslag_kopie");
        assert!(stelsel.chain("zorgtoeslag", "2", 3, 0).is_ok());
    }
}
