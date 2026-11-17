//! The gate that decides whether a law file may be enriched at all.
//!
//! Enrichment measures a translation against the statutory text in the file
//! it is given. When that text is not the law, the measurement says nothing
//! about the translation. That is not hypothetical: the checked-in
//! `wet_op_de_zorgtoeslag/2025-01-01.yaml` carries an article `1a` that the
//! official toestand does not have, an article 1 whose lettering and
//! definitions differ from the statute, and an article 2 missing the
//! threshold clause ("voorzover dat toetsingsinkomen het drempelinkomen te
//! boven gaat") that the calculation turns on.
//!
//! So this module reads the official BWB toestand and compares. It is
//! deliberately a step *before* the enricher and outside the agent: the
//! agent has no network, and this work belongs to harvesting anyway. It
//! stands here until the harvester takes it back.
//!
//! Two things come out of the same parse. The verdict per article, and the
//! structural placement of each article (chapter, division, paragraph with
//! their headings). The harvester's splitter starts at `artikel` and never
//! sees the containers, so that placement is lost today. It is the reason
//! an agent cannot tell that Awb 3:9 sits in "Afdeling 3.3 Advisering".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where an article sits in the document. Every level carries its number
/// and its heading, because the heading is condensed legal classification
/// written by the legislator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boek: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deel: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titeldeel: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<Level>,
}

impl Placement {
    /// True when no container encloses the article, which is normal for a
    /// short law.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Human-readable path, most general first.
    pub fn path(&self) -> String {
        [
            self.boek.as_ref().map(|l| ("Boek", l)),
            self.deel.as_ref().map(|l| ("Deel", l)),
            self.hoofdstuk.as_ref().map(|l| ("Hoofdstuk", l)),
            self.titeldeel.as_ref().map(|l| ("Titel", l)),
            self.afdeling.as_ref().map(|l| ("Afdeling", l)),
            self.paragraaf.as_ref().map(|l| ("Paragraaf", l)),
        ]
        .into_iter()
        .flatten()
        .map(|(kind, level)| match &level.heading {
            Some(h) => format!("{kind} {} {h}", level.number),
            None => format!("{kind} {}", level.number),
        })
        .collect::<Vec<_>>()
        .join(" > ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// One article as the official toestand has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArticle {
    pub number: String,
    /// Heading of the article itself, when it has one.
    pub heading: Option<String>,
    /// The statutory text, leden numbered as `1. `, `2. `, separated by a
    /// blank line, matching the convention the corpus already uses.
    pub text: String,
    /// The article addressed below its own level, because the harvested
    /// corpus splits there: a lid becomes its own entry (`3.2`) holding the
    /// chapeau, and each onderdeel another (`3.2.a`). Keys are the path
    /// after the article number, so `"2"` and `"2.a"`.
    pub parts: BTreeMap<String, String>,
    pub placement: Placement,
}

/// What the comparison says about one article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Text matches the official toestand after whitespace normalisation.
    Verified,
    /// Both have the article, the text differs.
    Drift { detail: String },
    /// The official toestand has it, the corpus file does not.
    Missing,
    /// The corpus file has it, the official toestand does not.
    Fabricated,
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Verified)
    }
}

/// The result of holding a corpus file against the official toestand.
#[derive(Debug, Default)]
pub struct GateReport {
    pub verdicts: BTreeMap<String, Verdict>,
    /// Placement per article number, from the official source.
    pub placements: BTreeMap<String, Placement>,
}

impl GateReport {
    /// The gate passes only when every article is verified. A single
    /// fabricated or drifted article means the file does not carry the law,
    /// and a translation measured against it proves nothing.
    pub fn passes(&self) -> bool {
        !self.verdicts.is_empty() && self.verdicts.values().all(Verdict::is_ok)
    }

    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut c = BTreeMap::new();
        for v in self.verdicts.values() {
            let key = match v {
                Verdict::Verified => "verified",
                Verdict::Drift { .. } => "drift",
                Verdict::Missing => "missing",
                Verdict::Fabricated => "fabricated",
            };
            *c.entry(key).or_insert(0) += 1;
        }
        c
    }
}

/// URL of the official toestand XML for a BWB id on a date.
pub fn toestand_url(bwb_id: &str, valid_from: &str) -> String {
    format!(
        "https://repository.officiele-overheidspublicaties.nl/bwb/{bwb_id}/{valid_from}_0/xml/{bwb_id}_{valid_from}_0.xml"
    )
}

/// Container elements, from most general to most specific. The harvester's
/// splitter starts at `artikel` and therefore never records any of these.
const CONTAINERS: &[&str] = &[
    "boek",
    "deel",
    "hoofdstuk",
    "titeldeel",
    "afdeling",
    "paragraaf",
];

/// Parse an official BWB toestand into articles with their placement.
pub fn parse_toestand(xml: &str) -> Result<Vec<SourceArticle>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("toestand is not XML: {e}"))?;
    let mut out = Vec::new();
    walk(doc.root_element(), &Placement::default(), &mut out);
    Ok(out)
}

fn walk(node: roxmltree::Node, inherited: &Placement, out: &mut Vec<SourceArticle>) {
    let tag = node.tag_name().name();

    if tag == "artikel" {
        if let Some(article) = read_article(node, inherited) {
            out.push(article);
        }
        return;
    }

    // A container contributes its own level to everything below it.
    let placement = if CONTAINERS.contains(&tag) {
        let mut p = inherited.clone();
        let level = read_level(node);
        match tag {
            "boek" => p.boek = level,
            "deel" => p.deel = level,
            "hoofdstuk" => p.hoofdstuk = level,
            "titeldeel" => p.titeldeel = level,
            "afdeling" => p.afdeling = level,
            "paragraaf" => p.paragraaf = level,
            _ => {}
        }
        p
    } else {
        inherited.clone()
    };

    for child in node.children().filter(roxmltree::Node::is_element) {
        walk(child, &placement, out);
    }
}

/// The `kop` of a container holds `nr` and usually `titel`.
fn read_level(node: roxmltree::Node) -> Option<Level> {
    let kop = node
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "kop")?;
    let number = child_text(kop, "nr").unwrap_or_default();
    let heading = child_text(kop, "titel").filter(|s| !s.is_empty());
    if number.is_empty() && heading.is_none() {
        return None;
    }
    Some(Level { number, heading })
}

fn read_article(node: roxmltree::Node, placement: &Placement) -> Option<SourceArticle> {
    let kop = node
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "kop");
    let number = kop.and_then(|k| child_text(k, "nr")).unwrap_or_default();
    if number.is_empty() {
        return None;
    }
    let heading = kop
        .and_then(|k| child_text(k, "titel"))
        .filter(|s| !s.is_empty());

    // Leden get their number back in front, which is how the corpus writes
    // them and what the per-lid accounting relies on. An article with a
    // single lid carries no number in the corpus convention, and adding one
    // here would report a formatting difference as drift on every such
    // article, drowning the real ones.
    let lid_nodes: Vec<_> = node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "lid")
        .collect();
    let single = lid_nodes.len() == 1;
    let leden: Vec<String> = lid_nodes
        .into_iter()
        .map(|lid| {
            let nr = child_text(lid, "lidnr").unwrap_or_default();
            let body = element_text_excluding(lid, &["lidnr", "meta-data"]);
            if nr.is_empty() || single {
                body
            } else {
                format!("{nr}. {body}")
            }
        })
        .filter(|s| !s.trim().is_empty())
        .collect();

    let text = if leden.is_empty() {
        element_text_excluding(node, &["kop", "meta-data"])
    } else {
        leden.join("\n\n")
    };

    Some(SourceArticle {
        number,
        heading,
        text: normalize_ws(&text),
        parts: read_parts(node),
        placement: placement.clone(),
    })
}

/// Address an article below its own level, the way the harvested corpus
/// does. A lid holds its chapeau without the list that hangs under it,
/// because that is what the corpus stores at `3.2` when it stores the items
/// separately at `3.2.a`.
fn read_parts(node: roxmltree::Node) -> BTreeMap<String, String> {
    let mut parts = BTreeMap::new();
    for lid in node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "lid")
    {
        let Some(nr) = child_text(lid, "lidnr").filter(|s| !s.is_empty()) else {
            continue;
        };
        parts.insert(
            nr.clone(),
            element_text_excluding(lid, &["lidnr", "meta-data", "lijst"]),
        );
        collect_items(lid, &nr, &mut parts);
    }
    parts
}

/// Walk the `lijst`/`li` nesting under a node, keying each item by the path
/// that leads to it. `li.nr` carries its own punctuation (`a.`, `1°`), which
/// the corpus strips.
fn collect_items(node: roxmltree::Node, prefix: &str, parts: &mut BTreeMap<String, String>) {
    for lijst in node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "lijst")
    {
        for li in lijst
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "li")
        {
            let Some(raw) = child_text(li, "li.nr") else {
                continue;
            };
            let label = raw.trim_end_matches(['.', ')', ' ']).to_string();
            if label.is_empty() {
                continue;
            }
            let key = format!("{prefix}.{label}");
            parts.insert(
                key.clone(),
                element_text_excluding(li, &["li.nr", "meta-data", "lijst"]),
            );
            collect_items(li, &key, parts);
        }
    }
}

fn child_text(node: roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .map(|c| normalize_ws(&all_text(c)))
}

fn all_text(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_text_excluding(node: roxmltree::Node, skip: &[&str]) -> String {
    let mut parts = Vec::new();
    for child in node.children() {
        if child.is_element() && skip.contains(&child.tag_name().name()) {
            continue;
        }
        if child.is_text() {
            if let Some(t) = child.text() {
                parts.push(t.to_string());
            }
        } else if child.is_element() {
            parts.push(all_text(child));
        }
    }
    normalize_ws(&parts.join(" "))
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip the presentation the harvester adds, so the comparison is about
/// what the text says rather than how it is written down.
///
/// Two harvester conventions are deliberate and must not read as drift.
/// Cross-references become markdown reference links (`[artikel 7, derde
/// lid][ref1]`) with a footer block of `[ref1]: https://…` lines, which is
/// how a reader gets a working link. And a lid is numbered `1 ` where this
/// module writes `1. `, which is a rendering choice on both sides.
///
/// Everything else stays significant. Accents, punctuation inside the
/// sentence and casing are differences on purpose: a translation is checked
/// word for word against this text, so a tolerance here would be a
/// tolerance there.
#[allow(clippy::expect_used)] // Static regexes that are guaranteed to be valid
fn normalize_for_comparison(s: &str) -> String {
    use std::sync::LazyLock;

    // `[ref1]: https://…` footer lines carry no statutory text.
    static FOOTER: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\[[^\]]+\]:\s*\S+").expect("valid regex"));
    // `[label][ref1]` and `[label](url)` keep only the label.
    static LINK: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"\[([^\]]*)\](?:\[[^\]]*\]|\([^)]*\))").expect("valid regex")
    });
    // A lid number is written `1 ` here and `1. ` there.
    static LIDNR: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(^|(?:[.;:] ))(\d{1,2})\.? ").expect("valid regex"));

    let without_footers = FOOTER.replace_all(s, "");
    let without_links = LINK.replace_all(&without_footers, "$1");
    let collapsed = normalize_ws(&without_links);
    // Joining XML elements with a space puts one in front of the
    // punctuation that follows them ("Zorgverzekeringswet , de"). That is an
    // artefact of reading the source, not a difference in the law.
    static SPACED_PUNCT: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r" +([,;:.])").expect("valid regex"));
    let tightened = SPACED_PUNCT.replace_all(&collapsed, "$1");
    LIDNR.replace_all(&tightened, "${1}${2}. ").into_owned()
}

/// The article number as a string, whatever YAML made of it.
///
/// `number: 1` is a number to the parser and `number: '1'` a string, and the
/// law means the same thing by both. Reading it with `as_str` alone silently
/// skips the numeric spelling, which in the rewrite meant an entry escaped
/// both the flatten guard and the carry-over: its translation was dropped
/// without a word. One reading, used everywhere an entry is identified.
fn article_number(article: &serde_yaml_ng::Value) -> Option<String> {
    match article.get("number") {
        Some(serde_yaml_ng::Value::String(s)) => Some(s.clone()),
        Some(serde_yaml_ng::Value::Null) | None => None,
        Some(other) => Some(
            serde_yaml_ng::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_string(),
        ),
    }
}

/// Compare a corpus law document against the official articles.
pub fn verify(corpus: &serde_yaml_ng::Value, official: &[SourceArticle]) -> GateReport {
    let mut report = GateReport::default();
    let mut official_by_number: BTreeMap<&str, &SourceArticle> = BTreeMap::new();
    for a in official {
        official_by_number.insert(a.number.as_str(), a);
        report
            .placements
            .insert(a.number.clone(), a.placement.clone());
    }

    let empty = Vec::new();
    let articles = corpus
        .get("articles")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .unwrap_or(&empty);

    let mut seen = std::collections::BTreeSet::new();
    for article in articles {
        let Some(number) = article_number(article) else {
            continue;
        };
        seen.insert(number.clone());
        let corpus_text = normalize_for_comparison(
            article
                .get("text")
                .and_then(serde_yaml_ng::Value::as_str)
                .unwrap_or_default(),
        );
        // The harvested corpus splits below article level, so a number
        // like `3.2.a` addresses lid 2 onderdeel a of article 3. Resolve by
        // longest matching article prefix: in a law whose articles are
        // themselves numbered `5.2`, that prefix wins over `5`.
        match resolve(&official_by_number, &number) {
            None => {
                report.verdicts.insert(number, Verdict::Fabricated);
            }
            Some((source, path)) => {
                let expected = if path.is_empty() {
                    Some(source.text.clone())
                } else {
                    source.parts.get(path).cloned()
                };
                let verdict = match expected {
                    None => Verdict::Fabricated,
                    Some(expected) => {
                        if corpus_text == normalize_for_comparison(&expected) {
                            Verdict::Verified
                        } else {
                            Verdict::Drift {
                                detail: describe_difference(&corpus_text, &expected),
                            }
                        }
                    }
                };
                report
                    .placements
                    .entry(number.clone())
                    .or_insert_with(|| source.placement.clone());
                report.verdicts.insert(number, verdict);
            }
        }
    }

    for a in official {
        // An article is present when the file carries it whole. A fragmented
        // corpus never carries the article number by itself, and then every
        // fragment the toestand has must be there: walking the *found* set
        // let a deleted lid pass, because a sibling that survived covered the
        // article for it. Deletion is exactly the drift this gate exists to
        // catch, so the walk goes over the expected set instead.
        if seen.iter().any(|s: &String| s == &a.number) {
            continue;
        }
        let fragments: Vec<&String> = seen
            .iter()
            .filter(|s| {
                s.strip_prefix(&a.number)
                    .is_some_and(|r| r.starts_with('.'))
            })
            .collect();
        if fragments.is_empty() {
            report.verdicts.insert(a.number.clone(), Verdict::Missing);
            continue;
        }
        for part in a.parts.keys() {
            let address = format!("{}.{}", a.number, part);
            if !fragments.contains(&&address) {
                report.verdicts.insert(address, Verdict::Missing);
            }
        }
    }

    report
}

/// The context sidecar: everything the official source knows about an
/// article that the law YAML has no field for.
///
/// This does not go into the law file. `machine_readable` and the article
/// object are `additionalProperties: false` on a released schema version,
/// so carrying placement there needs a version bump plus a corpus-wide
/// migration. Until that is decided, the worker reads this file and puts
/// the placement in front of the agent, which is what layer 3 of RFC-027
/// describes anyway: the worker assembles, the agent reads.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSidecar {
    pub bwb_id: String,
    pub valid_from: String,
    /// Article number to what the source says about it.
    pub articles: BTreeMap<String, ArticleContext>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArticleContext {
    /// Readable path, e.g. "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering".
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub path: String,
    #[serde(skip_serializing_if = "Placement::is_empty", default)]
    pub placement: Placement,
    /// The article's own heading, when the law gives it one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// What the gate said about this article before the rewrite.
    pub verdict: String,
}

/// Filename of the context sidecar, beside the law YAML.
pub const CONTEXT_SIDECAR: &str = ".source-context.yaml";

/// Rewrite a corpus law document so its `text` is the official text and its
/// article set is the official article set.
///
/// A `machine_readable` section is carried over by article number when the
/// article still exists. It is deliberately kept even when the text
/// drifted: dropping it would silently discard work, and the checks that
/// run afterwards are what decide whether the translation still holds
/// against the corrected text.
///
/// The rewrite refuses four situations instead of writing them out, all
/// shapes in which the write would destroy more than it corrects. An
/// empty official set never means the law lost its articles; it means the
/// fetch or the parse went wrong. A source set that shares not a single
/// article with the file (no verdict `Verified` or `Drift`) is a
/// different law, whatever the counts say — the shape a wrong BWB id of
/// similar size produces. A rewrite that keeps less than half of the
/// file's entries removes the majority of a law, which a rewrite never
/// legitimately does. And an entry that addresses a lid or onderdeel
/// while carrying `machine_readable` or `references` would be flattened
/// into its whole article, silently dropping that work, because the
/// carry-over goes by exact article number. All four need a human, not a
/// write; each refusal says what to do next.
///
/// Returns the rewritten document and the sidecar, or the reason the
/// rewrite is refused.
pub fn rewrite(
    corpus: &serde_yaml_ng::Value,
    official: &[SourceArticle],
    report: &GateReport,
) -> Result<(serde_yaml_ng::Value, ContextSidecar), String> {
    use serde_yaml_ng::{Mapping, Value};

    let empty_seq = Vec::new();
    let corpus_articles = corpus
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty_seq);
    let existing = corpus_articles.len();

    if official.is_empty() {
        return Err(format!(
            "the official toestand parsed to 0 articles while the file carries \
             {existing} entr{}; an empty source set means the fetch or the parse \
             failed (network error, changed response, wrong BWB id), not that the \
             law has no articles. The file is left as it is; check the bwb_id and \
             valid_from in the file and retry when the toestand is sound",
            if existing == 1 { "y" } else { "ies" }
        ));
    }

    // A source set that shares nothing with the file is a different law,
    // whatever the counts say. This is the wrong-BWB-id-of-similar-size
    // case: every entry fabricated, every official article missing, and a
    // size threshold alone would wave the substitution through.
    let overlap = report
        .verdicts
        .values()
        .any(|v| matches!(v, Verdict::Verified | Verdict::Drift { .. }));
    if existing > 0 && !overlap {
        return Err(format!(
            "not one of the file's {existing} entries matches an official \
             article (0 verified, 0 drifted); a source that shares nothing \
             with the file is a different law, so this points at a wrong \
             bwb_id or valid_from. The file is left as it is; run without \
             --rewrite to see the per-article verdicts",
        ));
    }

    if official.len() * 2 < existing {
        return Err(format!(
            "the rewrite would shrink the file from {existing} entries to {} \
             article(s); removing the majority of a law points at the wrong \
             source or at a per-lid fragmented corpus this whole-article \
             rewrite would flatten. The file is left as it is; run without \
             --rewrite to see the verdicts, and prune the file by hand if \
             the law really shrank this much",
            official.len()
        ));
    }

    // The carry-over below goes by exact article number, so an entry that
    // addresses a lid or onderdeel (`2.2`) is flattened into its whole
    // article (`2`) and anything it carries is dropped. Refuse when that
    // would discard work, whatever the entry counts are.
    let official_by_number: BTreeMap<&str, &SourceArticle> =
        official.iter().map(|a| (a.number.as_str(), a)).collect();
    for article in corpus_articles {
        let Some(number) = article_number(article) else {
            continue;
        };
        let Some((source, path)) = resolve(&official_by_number, &number) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        // Every key beyond `number` and `text` is authored work, which is
        // exactly what the carry-over keeps for a whole-article entry. A
        // fixed list of two here meant a lid-entry could still lose its
        // `placement`, its authored `url` or a key this gate has never seen,
        // silently: the guard was narrower than the thing it guards.
        let carried: Vec<String> = article
            .as_mapping()
            .into_iter()
            .flat_map(|m| m.keys())
            .filter_map(serde_yaml_ng::Value::as_str)
            .filter(|k| !matches!(*k, "number" | "text"))
            .map(str::to_string)
            .collect();
        if !carried.is_empty() {
            return Err(format!(
                "entry {number} addresses a lid or onderdeel of article {} and \
                 carries {}; the whole-article rewrite would flatten the entry \
                 and silently drop that work. The file is left as it is; move \
                 the work to the whole article first, or make the correction \
                 by hand",
                source.number,
                carried.join(" and ")
            ));
        }
    }

    // Everything the entry carries beyond `number` and `text` is authored
    // work: the translation, the `references` block the reference graph is
    // built from, the placement, the url. The rewrite owns the text and the
    // article set, nothing else, so every other key is carried over rather
    // than rebuilt from a fixed list. A key this rewrite has never seen is
    // kept for the same reason, and the schema check downstream decides
    // whether it belongs there.
    let mut carried: BTreeMap<String, Mapping> = BTreeMap::new();
    if let Some(seq) = corpus.get("articles").and_then(Value::as_sequence) {
        for article in seq {
            let Some(number) = article_number(article) else {
                continue;
            };
            let Some(map) = article.as_mapping() else {
                continue;
            };
            let kept: Mapping = map
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), Some("number") | Some("text")))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            carried.insert(number, kept);
        }
    }

    let base_url = corpus
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut sidecar = ContextSidecar {
        bwb_id: corpus
            .get("bwb_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        valid_from: corpus
            .get("valid_from")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        articles: BTreeMap::new(),
    };

    let mut articles = Vec::new();
    for source in official {
        let mut map = Mapping::new();
        map.insert(Value::from("number"), Value::from(source.number.as_str()));
        map.insert(Value::from("text"), Value::from(source.text.as_str()));
        if let Some(kept) = carried.get(&source.number) {
            for (key, value) in kept {
                map.insert(key.clone(), value.clone());
            }
        }
        // An article the file did not have yet, or one whose entry carried no
        // url, gets the deep link the corpus convention builds from the law's
        // own url.
        if !map.contains_key(Value::from("url")) {
            map.insert(
                Value::from("url"),
                Value::from(format!(
                    "{base_url}#Artikel{}",
                    source.number.replace(' ', "")
                )),
            );
        }
        articles.push(Value::Mapping(map));

        sidecar.articles.insert(
            source.number.clone(),
            ArticleContext {
                path: source.placement.path(),
                placement: source.placement.clone(),
                heading: source.heading.clone(),
                verdict: match report.verdicts.get(&source.number) {
                    Some(Verdict::Verified) => "verified".into(),
                    Some(Verdict::Drift { .. }) => "text replaced (had drifted)".into(),
                    Some(Verdict::Missing) => "text added (was absent)".into(),
                    Some(Verdict::Fabricated) | None => "new".into(),
                },
            },
        );
    }

    let mut doc = corpus.clone();
    if let Value::Mapping(map) = &mut doc {
        map.insert(Value::from("articles"), Value::Sequence(articles));
    }
    Ok((doc, sidecar))
}

/// A file staged next to its destination: the content is on disk and
/// synced, only the rename remains. Every failure path removes the temp
/// file again.
struct StagedWrite {
    tmp: std::path::PathBuf,
    dest: std::path::PathBuf,
}

impl StagedWrite {
    fn new(path: &std::path::Path, content: &str) -> Result<Self, String> {
        use std::io::Write;

        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("{} has no file name", path.display()))?;
        let tmp = parent.join(format!(".{file_name}.tmp"));

        (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()
        })()
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("writing {}: {e}", path.display())
        })?;

        Ok(Self {
            tmp,
            dest: path.to_path_buf(),
        })
    }

    fn commit(self) -> Result<(), String> {
        // On Windows, rename refuses when the destination exists.
        #[cfg(target_os = "windows")]
        if self.dest.exists() {
            std::fs::remove_file(&self.dest)
                .map_err(|e| format!("writing {}: {e}", self.dest.display()))?;
        }
        std::fs::rename(&self.tmp, &self.dest).map_err(|e| {
            let _ = std::fs::remove_file(&self.tmp);
            format!("writing {}: {e}", self.dest.display())
        })
    }
}

/// Write `content` to `path` through a temp file in the same directory and
/// an atomic rename, so an interruption never leaves a truncated file
/// behind. The harvester writes its YAML the same way (`yaml/writer.rs`),
/// for the same reason.
pub fn write_atomic(path: &std::path::Path, content: &str) -> Result<(), String> {
    StagedWrite::new(path, content)?.commit()
}

/// Write a law file and its companion so the law file changes last, and
/// only when everything else already succeeded. Both temp files are
/// written and synced before either rename; the companion is renamed
/// first. A failure at any point before the final rename leaves the law
/// file exactly as it was — the worst remaining case is a fresh companion
/// beside an unchanged law file, which the next run overwrites.
pub fn write_atomic_pair(
    precious: (&std::path::Path, &str),
    companion: (&std::path::Path, &str),
) -> Result<(), String> {
    let staged_precious = StagedWrite::new(precious.0, precious.1)?;
    let staged_companion = match StagedWrite::new(companion.0, companion.1) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&staged_precious.tmp);
            return Err(e);
        }
    };
    if let Err(e) = staged_companion.commit() {
        let _ = std::fs::remove_file(&staged_precious.tmp);
        return Err(e);
    }
    staged_precious.commit()
}

/// Split a corpus article number into the source article it belongs to and
/// the path within it. Longest prefix wins, so in a law with articles `5`
/// and `5.2` the number `5.2.1` resolves to article `5.2` part `1`.
fn resolve<'a>(
    official: &'a BTreeMap<&str, &SourceArticle>,
    number: &'a str,
) -> Option<(&'a SourceArticle, &'a str)> {
    if let Some(article) = official.get(number) {
        return Some((article, ""));
    }
    let mut best: Option<(&SourceArticle, &str)> = None;
    for (candidate, article) in official {
        if let Some(rest) = number.strip_prefix(*candidate)
            && let Some(path) = rest.strip_prefix('.')
            && best.is_none_or(|(cur, _)| cur.number.len() < candidate.len())
        {
            best = Some((article, path));
        }
    }
    best
}

/// A short, quotable description of how two texts differ: the first point
/// where they diverge, with a little context on both sides.
fn describe_difference(corpus: &str, source: &str) -> String {
    let common = corpus
        .char_indices()
        .zip(source.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((i, c), _)| i + c.len_utf8());

    let tail = |s: &str| -> String {
        let rest: String = s[common.min(s.len())..].chars().take(90).collect();
        if rest.is_empty() {
            "<einde>".to_string()
        } else {
            rest
        }
    };

    format!(
        "diverges after {common} chars; corpus has \"{}\", source has \"{}\"",
        tail(corpus),
        tail(source)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const XML: &str = r#"<toestand bwb-id="BWBR0000001">
      <wet-besluit>
        <wettekst>
          <hoofdstuk><kop><label>Hoofdstuk</label><nr>3</nr><titel>Besluiten</titel></kop>
            <afdeling><kop><label>Afdeling</label><nr>3.3</nr><titel>Advisering</titel></kop>
              <artikel><kop><label>Artikel</label><nr>3:5</nr></kop>
                <lid><lidnr>1</lidnr><al>In deze afdeling wordt verstaan onder adviseur: ...</al></lid>
              </artikel>
              <artikel><kop><label>Artikel</label><nr>3:9</nr></kop>
                <al>Het bestuursorgaan vergewist zich ervan dat het onderzoek zorgvuldig is verricht.</al>
              </artikel>
            </afdeling>
          </hoofdstuk>
          <artikel><kop><label>Artikel</label><nr>1</nr></kop>
            <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
            <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
          </artikel>
        </wettekst>
      </wet-besluit>
    </toestand>"#;

    #[test]
    fn placement_is_carried_down_from_the_containers() {
        let arts = parse_toestand(XML).unwrap();
        let by: BTreeMap<_, _> = arts.iter().map(|a| (a.number.as_str(), a)).collect();

        let a39 = by["3:9"];
        assert_eq!(a39.placement.hoofdstuk.as_ref().unwrap().number, "3");
        assert_eq!(
            a39.placement.afdeling.as_ref().unwrap().heading.as_deref(),
            Some("Advisering")
        );
        // This is the whole point: an agent reading 3:9 can now see that it
        // sits in the division on advice, and therefore that the adviseur
        // definition of 3:5 scopes it.
        assert_eq!(
            a39.placement.path(),
            "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering"
        );

        // An article outside every container has an empty placement.
        assert!(by["1"].placement.is_empty());
    }

    #[test]
    fn leden_keep_their_numbering() {
        let arts = parse_toestand(XML).unwrap();
        let a1 = arts.iter().find(|a| a.number == "1").unwrap();
        assert_eq!(a1.text, "1. Eerste lid. 2. Tweede lid.");

        // A single-lid article carries no number, matching the corpus
        // convention. Numbering it would report formatting as drift.
        let a39 = arts.iter().find(|a| a.number == "3:5").unwrap();
        assert!(
            a39.text.starts_with("In deze afdeling"),
            "single lid must not be numbered: {:?}",
            a39.text
        );
    }

    #[test]
    fn an_article_the_source_does_not_have_is_fabricated() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Tweede lid.\n  - number: 1a\n    text: verzonnen\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        assert_eq!(report.verdicts["1a"], Verdict::Fabricated);
        assert!(!report.passes());
    }

    #[test]
    fn an_article_the_corpus_lacks_is_missing() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Tweede lid.\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        assert_eq!(report.verdicts["3:9"], Verdict::Missing);
    }

    #[test]
    fn drift_names_the_point_of_divergence() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Een heel ander tweede lid.\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let Verdict::Drift { detail } = &report.verdicts["1"] else {
            panic!("expected drift, got {:?}", report.verdicts["1"]);
        };
        assert!(detail.contains("Een heel ander"), "{detail}");
    }

    #[test]
    fn rewrite_replaces_the_text_and_keeps_the_translation() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001
articles:
  - number: '1'
    text: iets heel anders
    url: u
    machine_readable: {a: 1}
  - number: 1a
    text: verzonnen artikel
    url: u
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let (doc, sidecar) = rewrite(&corpus, &official, &report).unwrap();

        let arts = doc.get("articles").unwrap().as_sequence().unwrap();
        let numbers: Vec<&str> = arts
            .iter()
            .filter_map(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str))
            .collect();
        // The fabricated article is gone and the ones the law has are there.
        assert!(!numbers.contains(&"1a"), "{numbers:?}");
        assert!(numbers.contains(&"3:9"), "{numbers:?}");

        let a1 = arts
            .iter()
            .find(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str) == Some("1"))
            .unwrap();
        assert_eq!(
            a1.get("text").and_then(serde_yaml_ng::Value::as_str),
            Some("1. Eerste lid. 2. Tweede lid.")
        );
        // Existing work is carried over rather than silently discarded; the
        // checks that follow decide whether it still holds.
        assert!(a1.get("machine_readable").is_some());

        assert_eq!(
            sidecar.articles["3:9"].path,
            "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering"
        );
        assert_eq!(sidecar.articles["1"].verdict, "text replaced (had drifted)");
    }

    #[test]
    fn rewrite_keeps_every_authored_key_on_the_article() {
        // `references` feeds the cross-law reference graph, and `placement`
        // is authored structure. A rewrite that rebuilds the entry from a
        // fixed list of keys drops both without saying so.
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001
articles:
  - number: '1'
    text: iets heel anders
    url: u
    machine_readable: {a: 1}
    placement: {hoofdstuk: {number: '1'}}
    references:
      - bwb_id: BWBR0000002
        artikel: '8'
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let (doc, _) = rewrite(&corpus, &official, &report).unwrap();

        let arts = doc.get("articles").unwrap().as_sequence().unwrap();
        let a1 = arts
            .iter()
            .find(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str) == Some("1"))
            .unwrap();
        assert_eq!(
            a1.get("text").and_then(serde_yaml_ng::Value::as_str),
            Some("1. Eerste lid. 2. Tweede lid.")
        );
        assert!(a1.get("machine_readable").is_some(), "{a1:?}");
        assert!(a1.get("placement").is_some(), "{a1:?}");
        let refs = a1
            .get("references")
            .and_then(serde_yaml_ng::Value::as_sequence)
            .unwrap_or_else(|| panic!("references dropped: {a1:?}"));
        assert_eq!(
            refs[0]
                .get("artikel")
                .and_then(serde_yaml_ng::Value::as_str),
            Some("8")
        );
        // The url the entry carried wins over the generated deep link.
        assert_eq!(
            a1.get("url").and_then(serde_yaml_ng::Value::as_str),
            Some("u")
        );

        // An article the file did not have yet still gets a deep link.
        let a39 = arts
            .iter()
            .find(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str) == Some("3:9"))
            .unwrap();
        assert_eq!(
            a39.get("url").and_then(serde_yaml_ng::Value::as_str),
            Some("https://wetten.overheid.nl/BWBR0000001#Artikel3:9")
        );
    }

    #[test]
    fn rewrite_refuses_an_empty_official_set() {
        // A toestand that parses but holds no articles: what a changed
        // response format or a wrong BWB id produces. Yesterday this shape
        // erased a complete law from the corpus.
        let official =
            parse_toestand(r#"<toestand bwb-id="BWBR0000001"><wettekst/></toestand>"#).unwrap();
        assert!(official.is_empty());
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: Eerste artikel.\n    machine_readable: {a: 1}\n  - number: '2'\n    text: Tweede artikel.\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let err = rewrite(&corpus, &official, &report).unwrap_err();
        assert!(err.contains("0 articles"), "{err}");
        assert!(err.contains("2 entries"), "{err}");
        // The message must point at the real causes, not at the law.
        assert!(err.contains("wrong BWB id"), "{err}");
    }

    #[test]
    fn rewrite_refuses_to_remove_the_majority_of_a_law() {
        // A per-lid fragmented corpus: six entries that all belong to the
        // single official article. Writing the whole-article form would
        // flatten the leden and drop every extra field, which is what
        // happened to the Awir (329 entries became 87 bare articles).
        let official = parse_toestand(
            r#"<toestand bwb-id="BWBR0000001"><wettekst>
              <artikel><kop><label>Artikel</label><nr>1</nr></kop>
                <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
                <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
              </artikel>
            </wettekst></toestand>"#,
        )
        .unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - {number: '1.1', text: Eerste lid.}
  - {number: '1.2', text: Tweede lid.}
  - {number: '1.2.a', text: onderdeel a}
  - {number: '1.2.b', text: onderdeel b}
  - {number: '1.2.c', text: onderdeel c}
  - {number: '1.2.d', text: onderdeel d}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let err = rewrite(&corpus, &official, &report).unwrap_err();
        assert!(err.contains("6 entries"), "{err}");
        assert!(err.contains("1 article(s)"), "{err}");
    }

    #[test]
    fn rewrite_still_accepts_keeping_exactly_half() {
        // Two fabricated entries next to two real articles: dropping the
        // fabrications keeps half of the file, and that is a correction,
        // not a demolition.
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - {number: '1', text: 1. Eerste lid. 2. Tweede lid.}
  - {number: '3:9', text: iets}
  - {number: 1a, text: verzonnen}
  - {number: 1b, text: ook verzonnen}
  - {number: 1c, text: nog een}
  - {number: 1d, text: en nog een}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        // XML carries three official articles; 6 entries -> 3 is exactly
        // half and passes the threshold.
        assert!(rewrite(&corpus, &official, &report).is_ok());
    }

    #[test]
    fn rewrite_refuses_a_source_set_that_shares_nothing_with_the_file() {
        // A wrong BWB id pointing at a law of similar size: every entry
        // fabricated, every official article missing, and the counts even.
        // A size threshold alone waves the substitution through and every
        // machine_readable is gone.
        let official = parse_toestand(
            r#"<toestand bwb-id="BWBR0000001"><wettekst>
              <artikel><kop><label>Artikel</label><nr>1</nr></kop><al>Een.</al></artikel>
              <artikel><kop><label>Artikel</label><nr>2</nr></kop><al>Twee.</al></artikel>
            </wettekst></toestand>"#,
        )
        .unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - {number: '10', text: Tien., machine_readable: {a: 1}}
  - {number: '11', text: Elf., machine_readable: {b: 2}}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        // The premise: zero overlap while the counts match.
        assert!(
            !report
                .verdicts
                .values()
                .any(|v| matches!(v, Verdict::Verified | Verdict::Drift { .. }))
        );
        let err = rewrite(&corpus, &official, &report).unwrap_err();
        assert!(err.contains("different law"), "{err}");
        // The refusal must tell the user the next step.
        assert!(err.contains("without --rewrite"), "{err}");
    }

    #[test]
    fn rewrite_refuses_to_flatten_a_part_entry_that_carries_work() {
        // Three entries against two official articles: under the size
        // threshold, but entry 2.2 addresses a lid and carries a
        // machine_readable that the by-exact-number carry-over would drop.
        let official = parse_toestand(
            r#"<toestand bwb-id="BWBR0000001"><wettekst>
              <artikel><kop><label>Artikel</label><nr>1</nr></kop><al>Een.</al></artikel>
              <artikel><kop><label>Artikel</label><nr>2</nr></kop>
                <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
                <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
              </artikel>
            </wettekst></toestand>"#,
        )
        .unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - {number: '1', text: Een.}
  - {number: '2.1', text: Eerste lid.}
  - {number: '2.2', text: Tweede lid., machine_readable: {a: 1}}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let err = rewrite(&corpus, &official, &report).unwrap_err();
        assert!(err.contains("entry 2.2"), "{err}");
        assert!(err.contains("machine_readable"), "{err}");
        assert!(err.contains("by hand"), "{err}");
    }

    #[test]
    fn part_entries_without_work_do_not_block_the_rewrite() {
        // The same fragmentation, but the part entries carry nothing that
        // the flattening would lose: the text survives inside the whole
        // article, so the rewrite may proceed.
        let official = parse_toestand(
            r#"<toestand bwb-id="BWBR0000001"><wettekst>
              <artikel><kop><label>Artikel</label><nr>1</nr></kop><al>Een.</al></artikel>
              <artikel><kop><label>Artikel</label><nr>2</nr></kop>
                <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
                <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
              </artikel>
            </wettekst></toestand>"#,
        )
        .unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - {number: '1', text: Een., machine_readable: {a: 1}}
  - {number: '2.1', text: Eerste lid.}
  - {number: '2.2', text: Tweede lid.}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let (doc, _) = rewrite(&corpus, &official, &report).unwrap();
        let arts = doc.get("articles").unwrap().as_sequence().unwrap();
        assert_eq!(arts.len(), 2);
        // Work on a whole-article entry is still carried over.
        assert!(arts[0].get("machine_readable").is_some());
    }

    #[test]
    fn a_failing_companion_write_leaves_the_precious_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let precious = dir.path().join("law.yaml");
        std::fs::write(&precious, "oud").unwrap();
        // A directory where the companion file should land makes its
        // rename fail after both temp files were staged.
        let companion = dir.path().join("sidecar.yaml");
        std::fs::create_dir(&companion).unwrap();

        let err = write_atomic_pair((&precious, "nieuw"), (&companion, "context")).unwrap_err();
        assert!(err.contains("sidecar.yaml"), "{err}");
        assert_eq!(std::fs::read_to_string(&precious).unwrap(), "oud");
        // No temp litter either.
        assert!(!dir.path().join(".law.yaml.tmp").exists());
        assert!(!dir.path().join(".sidecar.yaml.tmp").exists());
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        std::fs::write(&path, "oud").unwrap();
        // A stale temp file from an earlier interrupted run is harmless.
        std::fs::write(dir.path().join(".law.yaml.tmp"), "afgekapt").unwrap();

        write_atomic(&path, "nieuw").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "nieuw");
        assert!(!dir.path().join(".law.yaml.tmp").exists());
    }

    const FRAGMENTED_XML: &str = r#"<toestand bwb-id="BWBR0000002">
      <wettekst>
        <artikel><kop><label>Artikel</label><nr>3</nr></kop>
          <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
          <lid><lidnr>2</lidnr><al>Chapeau van het tweede lid:</al>
            <lijst><li><li.nr>a.</li.nr><al>onderdeel a;</al></li>
                   <li><li.nr>b.</li.nr><al>onderdeel b.</al></li></lijst>
          </lid>
        </artikel>
        <artikel><kop><label>Artikel</label><nr>5.2</nr></kop>
          <lid><lidnr>1</lidnr><al>Artikel vijf punt twee, eerste lid.</al></lid>
        </artikel>
      </wettekst>
    </toestand>"#;

    #[test]
    fn a_lid_holds_its_chapeau_and_the_items_stand_apart() {
        let arts = parse_toestand(FRAGMENTED_XML).unwrap();
        let a3 = arts.iter().find(|a| a.number == "3").unwrap();
        // The corpus stores the chapeau at `3.2` and the items at `3.2.a`,
        // so the lid must not swallow its own list.
        assert_eq!(a3.parts["2"], "Chapeau van het tweede lid:");
        assert_eq!(a3.parts["2.a"], "onderdeel a;");
        assert_eq!(a3.parts["2.b"], "onderdeel b.");
        assert_eq!(a3.parts["1"], "Eerste lid.");
    }

    #[test]
    fn a_fragmented_corpus_verifies_against_the_parts() {
        let official = parse_toestand(FRAGMENTED_XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '3.1'
    text: Eerste lid.
  - number: '3.2'
    text: 'Chapeau van het tweede lid:'
  - number: 3.2.a
    text: onderdeel a;
  - number: 3.2.b
    text: onderdeel b.
  - number: 5.2.1
    text: Artikel vijf punt twee, eerste lid.
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        assert!(
            report.verdicts.values().all(Verdict::is_ok),
            "expected every fragment to verify: {:?}",
            report.verdicts
        );
    }

    #[test]
    fn the_longest_article_prefix_wins() {
        // `5.2.1` is article `5.2` lid 1, not article `5` lid 2 item 1.
        let official = parse_toestand(FRAGMENTED_XML).unwrap();
        let by: BTreeMap<&str, &SourceArticle> =
            official.iter().map(|a| (a.number.as_str(), a)).collect();
        let (article, path) = resolve(&by, "5.2.1").unwrap();
        assert_eq!(article.number, "5.2");
        assert_eq!(path, "1");
    }

    #[test]
    fn an_article_covered_only_by_its_fragments_is_not_missing() {
        let official = parse_toestand(FRAGMENTED_XML).unwrap();
        let corpus: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("articles:\n  - number: '3.1'\n    text: Eerste lid.\n")
                .unwrap();
        let report = verify(&corpus, &official);
        assert!(!report.verdicts.contains_key("3"), "{:?}", report.verdicts);
        assert_eq!(report.verdicts["5.2"], Verdict::Missing);
    }

    #[test]
    fn a_fragment_the_toestand_has_and_the_file_lost_is_missing() {
        // Walking the found set let this pass: article 3 counted as covered
        // because lid 1 survived, and the deleted lid 2 with its two items
        // was never looked for. A lid that disappears from a file is the
        // drift a text gate exists to catch, so each expected fragment is
        // now checked by its own address.
        let official = parse_toestand(FRAGMENTED_XML).unwrap();
        let corpus: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("articles:\n  - number: '3.1'\n    text: Eerste lid.\n")
                .unwrap();
        let report = verify(&corpus, &official);
        assert_eq!(
            report.verdicts["3.2"],
            Verdict::Missing,
            "{:?}",
            report.verdicts
        );
        assert_eq!(report.verdicts["3.2.a"], Verdict::Missing);
        assert_eq!(report.verdicts["3.2.b"], Verdict::Missing);
        assert!(!report.passes(), "a file missing a lid must not pass");
    }

    #[test]
    fn a_lid_entry_carrying_placement_is_refused_rather_than_flattened() {
        // The guard listed `machine_readable` and `references` by name, so a
        // lid-entry carrying only `placement` or an authored `url` was
        // flattened into its whole article and lost them without a word.
        let official = parse_toestand(FRAGMENTED_XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '3.1'
    text: Eerste lid.
    placement: {hoofdstuk: {number: '1'}}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let err = rewrite(&corpus, &official, &report)
            .expect_err("a lid-entry carrying authored work must be refused");
        assert!(err.contains("placement"), "got {err}");
    }

    #[test]
    fn an_unquoted_article_number_keeps_its_translation() {
        // `number: 1` is a YAML number, `number: '1'` a string, and the law
        // means the same by both. Reading it with `as_str` alone made the
        // entry invisible to the carry-over, so the rewrite replaced it with
        // a bare article and the translation was gone without a word. The
        // same slip made it invisible to the flatten guard.
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001
articles:
  - number: 1
    text: oude tekst
    machine_readable: {a: 1}
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let (doc, _) = rewrite(&corpus, &official, &report).unwrap();
        let arts = doc.get("articles").unwrap().as_sequence().unwrap();
        let a1 = arts
            .iter()
            .find(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str) == Some("1"))
            .expect("article 1");
        assert!(
            a1.get("machine_readable").is_some(),
            "the translation must survive an unquoted number: {a1:?}"
        );
    }

    #[test]
    fn toestand_url_matches_the_repository_layout() {
        assert_eq!(
            toestand_url("BWBR0018451", "2025-01-01"),
            "https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/2025-01-01_0/xml/BWBR0018451_2025-01-01_0.xml"
        );
    }
}
