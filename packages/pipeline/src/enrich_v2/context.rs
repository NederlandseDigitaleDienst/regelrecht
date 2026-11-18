//! What the worker puts in front of the agent before it translates a window.
//!
//! The prompt used to hand over the law file and the sentence "process ONLY
//! these articles". Everything else the agent had to find for itself, and
//! whether it looked was not observable. Two of the three things it needed are
//! not findable by reading the window at all.
//!
//! The first is where an article sits. A definition at the head of an afdeling
//! governs that afdeling without any article referring to it, so which
//! provisions bear on article 8 follows from the containers around article 8.
//! That is what `placement` is for, and [`super::assemble`] already carries it
//! per article as `path`.
//!
//! The second is what modifies the window from outside it. Article 10 may say
//! "in afwijking van artikel 8", and then the influence runs from 10 to 8 while
//! the reference points the other way. An agent given only article 8 translates
//! a rule that the law does not have. Finding those means scanning the whole
//! law, which is cheap because the text is already here, and it is why this
//! module resolves inbound references while cross-law resolution stays narrow.
//!
//! Not every inbound reference belongs in the brief. "Het bedrag, bedoeld in
//! artikel 8" uses article 8 and leaves it alone. The connectives that mark a
//! modifying reference are the ones the coverage check already knows, which is
//! not a coincidence: both are asking where a rule stops being what it looks
//! like.
//!
//! The third is what the window cites from other statutes. Measured on the
//! zorgtoeslag window of 1 and 2: the agent spent 28 turns and 2.7M
//! input-equivalent tokens — 17% of that run — on `ls`, `grep`, `awk` and a
//! `python3` that died on a missing `yaml` module, all of it to find out which
//! article numbers the Zorgverzekeringswet has and what article 68b, vijfde
//! lid, says. None of that is a judgement call: the `references` block on the
//! article already carries the BWB number and the article number of every
//! citation, and those laws sit in the same corpus. So the worker looks them
//! up and the agent reads them, the same trade this module already makes for
//! placement and for definitions.
//!
//! What it may not do is grow without bound. The brief rides along on every
//! turn, so a window citing twenty articles would pay for those twenty
//! articles a hundred times over. Hence a budget, and hence the rule that the
//! brief names what it left out: a truncation the reader cannot see is worse
//! than one they can, because it reads as "this is all there is".

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

use super::assemble::{AssembledArticle, DefinitionScope, LawContext, Placement, split_number};

/// File the worker writes beside the law for the agent to read.
///
/// Markdown rather than YAML because it is read and never parsed, and a dot
/// prefix so a corpus checkout does not gain a stray document.
pub const CONTEXT_BRIEF: &str = ".enrichment-context.md";

/// Words that make an inbound reference modifying rather than merely using.
///
/// A shorter list than the coverage check's, because that one asks whether a
/// model has branches and this one asks whether one article bends another.
/// "Tenzij" and "mits" qualify a rule inside its own article; they say nothing
/// about an article elsewhere.
const MODIFYING: &[&str] = &[
    "in afwijking van",
    "onverminderd",
    "in aanvulling op",
    "niet van toepassing",
    "geldt niet",
    "blijft buiten toepassing",
    "behoudens",
    "met dien verstande",
    "voor de toepassing van",
];

/// One article bending another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    /// The article doing the modifying.
    pub from: String,
    /// The article in the window that it modifies.
    pub to: String,
    /// The connective that made this a modification.
    pub connective: String,
    /// The sentence it appeared in, so the agent can judge for itself.
    pub sentence: String,
}

/// Split a text into sentences, crudely and on purpose.
///
/// A statutory sentence ends at a full stop followed by a space, and the
/// abbreviations that would trip this up ("art.", "jo.", "e.d.") do not change
/// which connective sits next to which article number. Getting a boundary
/// wrong widens or narrows the quoted sentence and never changes the verdict.
fn sentences(text: &str) -> Vec<String> {
    strip_reference_links(text)
        .split_inclusive(['.', ';'])
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Drop the markdown reference-link machinery the harvester leaves in the text.
///
/// Two separate hazards. The footer lines (`[ref1]: https://wetten.nl/...`)
/// contain full stops inside a URL, so sentence splitting cuts through them and
/// the halves end up quoted as statutory text in the brief. And the fragment
/// `#Artikel1` reads as a reference to article 1 to anything scanning for the
/// word, which is how article 1 of the Awir acquired two modifiers it does not
/// have.
fn strip_reference_links(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim_start();
            !(t.starts_with('[') && t.contains("]: ") && t.contains("://"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Article numbers referred to in one sentence.
///
/// Handles "artikel 8", "artikelen 8 en 9", "artikel 8, tweede lid" and the
/// letter-suffixed numbering the corpus uses ("artikel 3a"). The lid is
/// dropped: the node is the article, and the address only decides which
/// fragment is offered, which happens elsewhere.
pub fn referenced_articles(sentence: &str) -> Vec<String> {
    let lower = sentence.to_lowercase();
    let mut out = Vec::new();
    let mut rest = lower.as_str();
    while let Some(pos) = rest.find("artikel") {
        let after = &rest[pos + "artikel".len()..];
        // `#artikel1` in a URL is not a reference. A real one separates the
        // word from the number, with a space or the plural ending.
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            rest = after;
            continue;
        }
        // "artikel 8 van de Zorgverzekeringswet" points outside this law, and
        // this brief is about what one law does to itself. Cross-law edges are
        // the work queue's business, not the brief's.
        if is_external_reference(after) {
            rest = after;
            continue;
        }
        // "artikelen 8 en 9" enumerates; "artikel 8" does not.
        let tail = after.strip_prefix("en").unwrap_or(after);
        let mut scan = tail;
        loop {
            let trimmed = scan.trim_start_matches([' ', ',']);
            let taken: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                .collect();
            let number: String = taken
                .chars()
                .take_while(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                .collect();
            if number.is_empty() || !number.starts_with(|c: char| c.is_ascii_digit()) {
                break;
            }
            out.push(number.clone());
            let consumed = trimmed.len() - trimmed[number.len()..].len();
            scan = &trimmed[consumed..];
            // Only "en" or a comma continues an enumeration.
            let next = scan.trim_start();
            if let Some(after_en) = next.strip_prefix("en ") {
                scan = after_en;
            } else if next.starts_with(", ") && next[2..].starts_with(|c: char| c.is_ascii_digit())
            {
                scan = &next[2..];
            } else {
                break;
            }
        }
        rest = after;
    }
    out.sort();
    out.dedup();
    out
}

/// Whether what follows an article number sends it to another law.
///
/// "van de", "van het" and "van die" introduce the statute the article belongs
/// to. Without this the brief claims that an article of some other law modifies
/// an article here, which is worse than saying nothing: it is a relation the
/// reader cannot check against the file in front of them.
fn is_external_reference(after_word: &str) -> bool {
    let tail: String = after_word
        .chars()
        .skip_while(|c| c.is_whitespace() || c.is_ascii_digit() || *c == ',')
        .take(60)
        .collect();
    let tail = tail.trim_start();
    for lead in [
        "van de ",
        "van het ",
        "van die ",
        "van deze wet",
        "van dat ",
    ] {
        if tail.starts_with(lead) {
            // "van deze wet" points back here, so it is not external.
            return !tail.starts_with("van deze wet");
        }
    }
    // A lid qualifier may sit in between: "artikel 8, tweede lid, van de Awir".
    if let Some(rest) = tail.strip_prefix("tweede lid").or_else(|| {
        ["eerste lid", "derde lid", "vierde lid", "vijfde lid"]
            .iter()
            .find_map(|l| tail.strip_prefix(l))
    }) {
        let rest = rest.trim_start_matches([',', ' ']);
        return rest.starts_with("van de ")
            || rest.starts_with("van het ")
            || rest.starts_with("van die ");
    }
    false
}

/// Articles outside the window that modify articles inside it.
///
/// Scans the whole law: an article that bends the window can sit anywhere, and
/// document order says nothing about it. Self-references are dropped, because
/// an article qualifying its own lid is already in front of the agent.
#[must_use]
pub fn inbound_modifiers(law: &LawContext, window: &[String]) -> Vec<Inbound> {
    let target: std::collections::BTreeSet<&str> = window.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    for article in &law.articles {
        if target.contains(article.number.as_str()) {
            continue;
        }
        for sentence in sentences(&article.text) {
            let lower = sentence.to_lowercase();
            let Some(connective) = MODIFYING.iter().find(|c| lower.contains(**c)) else {
                continue;
            };
            for number in referenced_articles(&sentence) {
                if !target.contains(number.as_str()) {
                    continue;
                }
                out.push(Inbound {
                    from: article.number.clone(),
                    to: number,
                    connective: (*connective).to_owned(),
                    sentence: sentence.clone(),
                });
            }
        }
    }
    out
}

/// How much of one cited article the brief reproduces.
///
/// Two and a half thousand characters is roughly a lid or three of statutory
/// text. It was chosen against what the run actually went looking for: the
/// eight articles the zorgtoeslag window cites run from 1.2 KB (Zvw 18e) to
/// 9.9 KB (Zvw 1, a definition article with 29 onderdelen), and the median
/// sits just under 4 KB. A cap here therefore lands whole on the small ones
/// and cuts the two big definition articles, which is the right way round: an
/// agent that needs onderdeel z of a definition article knows to ask, whereas
/// one that never saw article 18e at all does not know it is missing anything.
const CROSS_LAW_ARTICLE_CHARS: usize = 2_500;

/// How much of the brief the whole cross-law section may take.
///
/// Sixteen thousand characters is about four thousand tokens, and the brief is
/// re-read on every turn of the window. At the ~110 turns the measured run took
/// that is some 440k input-equivalents added, against the 2.7M the same run
/// spent finding this material by hand. The ratio is what justifies the number,
/// not the number itself: raise it and the arithmetic turns.
const CROSS_LAW_TOTAL_CHARS: usize = 16_000;

/// An article of another law that the window cites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedArticle {
    /// BWB identifier of the law it belongs to, as the reference gives it.
    pub bwb_id: String,
    /// `$id` of that law, empty when the corpus does not have it.
    pub law_id: String,
    /// Article number within that law.
    pub artikel: String,
    /// The window articles that cite it, in document order.
    pub cited_by: Vec<String>,
    /// What the brief can show of it.
    pub body: CitedBody,
}

/// What became of one citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitedBody {
    /// Text the brief reproduces, with however many fragments the budget cut.
    Text {
        /// The article, assembled from its corpus entries.
        text: String,
        /// Entries left out because the per-article budget ran out.
        dropped: usize,
    },
    /// The corpus has no file for this BWB number, so nothing can be shown and
    /// nothing can be found by looking either.
    NotInCorpus,
    /// The corpus has it, but the brief's budget was already spent.
    OverBudget,
    /// The corpus has a file for it that could not be read or understood.
    ///
    /// Distinct from `NotInCorpus` because the responses differ: an absent law
    /// is absent and looking is wasted, while a file that would not parse is a
    /// defect in the corpus that someone has to fix. Before this variant the
    /// three failure arms skipped the citation entirely, so the law appeared
    /// neither in the brief nor under "Cited but not reproduced", under a
    /// heading that promises the list is exhaustive.
    Unreadable(String),
}

/// `(bwb_id, artikel)` for every reference the window's entries carry.
///
/// Reads the `references` block the harvester writes rather than the prose,
/// because that block is already resolved: it names the statute by BWB number
/// and the article by number, where the sentence says "artikel 68b, vijfde
/// lid, van de Zorgverzekeringswet" and leaves the resolving to whoever reads
/// it. References into this same law are dropped: that text is in front of the
/// agent already, and the inbound scan above covers what it does to the window.
#[must_use]
pub fn window_citations(
    doc: &Value,
    known: &[String],
    window: &[String],
    own_bwb: &str,
) -> Vec<(String, String, String)> {
    let target: BTreeSet<&str> = window.iter().map(String::as_str).collect();
    let empty = Vec::new();
    let mut out = Vec::new();
    for entry in doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty)
    {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        let article = split_number(known, number).0;
        let article = if article.is_empty() { number } else { article };
        if !target.contains(article) {
            continue;
        }
        for reference in entry
            .get("references")
            .and_then(Value::as_sequence)
            .unwrap_or(&empty)
        {
            let Some(bwb) = reference.get("bwb_id").and_then(Value::as_str) else {
                continue;
            };
            // A reference without an article number points at the statute as a
            // whole. Reproducing a whole law is not on offer, and naming it
            // without text would only invite a hunt.
            let Some(artikel) = reference.get("artikel").and_then(Value::as_str) else {
                continue;
            };
            if bwb.eq_ignore_ascii_case(own_bwb) {
                continue;
            }
            out.push((bwb.to_owned(), artikel.to_owned(), article.to_owned()));
        }
    }
    out
}

/// How much of a corpus file is read to learn which law it is.
///
/// `bwb_id` and `valid_from` sit in the first handful of lines and the files
/// run to hundreds of kilobytes, so reading them whole to answer one question
/// is what turned a lookup into twenty seconds of I/O per window on a corpus of
/// this size. Two kilobytes is generous for a header that is six lines of
/// scalars.
const HEAD_BYTES: usize = 2_048;

/// `(bwb_id, valid_from)` from the head of a corpus file.
///
/// A line scan rather than a YAML parse, because a fixed-size read can land in
/// the middle of a folded scalar and a document truncated there does not parse.
/// Reading further to make it parse is the cost this is avoiding, and failing
/// to parse would say "this file names no law", which is the one answer that
/// must not be wrong: it turns a law that is in the corpus into one the brief
/// tells the agent to stop looking for. Both keys are top-level scalars in
/// every harvested file, so column zero is the whole grammar needed.
fn read_identity(path: &Path) -> Option<(String, String)> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; HEAD_BYTES];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    let text = String::from_utf8_lossy(&buffer).into_owned();

    let scalar = |key: &str| -> Option<String> {
        text.lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|rest| rest.trim().trim_matches(['\'', '"']).to_owned())
    };
    Some((
        scalar("bwb_id:")?,
        scalar("valid_from:").unwrap_or_default(),
    ))
}

/// Corpus files by BWB number, stopping as soon as every wanted number is found.
///
/// Walks directory by directory because the corpus keeps one law per directory:
/// once a directory has yielded a BWB number, every version of that law has
/// been seen and the number can come off the list, which is what lets the walk
/// stop early instead of always traversing the whole corpus.
///
/// Where a law has several version files the one in force at `valid_from` wins,
/// because an agent shown a different redaction than the one it is translating
/// against is worse off than one shown nothing: it cannot see that the text
/// moved.
fn locate_laws(
    root: &Path,
    wanted: &BTreeSet<String>,
    valid_from: &str,
) -> BTreeMap<String, PathBuf> {
    let mut remaining: BTreeSet<&str> = wanted.iter().map(String::as_str).collect();
    let mut best: BTreeMap<String, (String, PathBuf)> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if remaining.is_empty() {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut found_here: BTreeSet<String> = BTreeSet::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let Some((bwb, version)) = read_identity(&path) else {
                continue;
            };
            if !remaining.contains(bwb.as_str()) {
                continue;
            }
            found_here.insert(bwb.clone());
            match best.get(&bwb) {
                Some((current, _)) if !better_version(current, &version, valid_from) => {}
                _ => {
                    best.insert(bwb, (version, path));
                }
            }
        }
        for bwb in &found_here {
            remaining.remove(bwb.as_str());
        }
    }
    best.into_iter().map(|(k, (_, p))| (k, p)).collect()
}

/// Whether `candidate` is a better match for `wanted` than `current`.
///
/// In force at the date beats not in force; among those in force the latest
/// wins; among those not yet in force the earliest, so a law whose only
/// versions post-date the window still yields the nearest one rather than
/// nothing.
fn better_version(current: &str, candidate: &str, wanted: &str) -> bool {
    let in_force = |v: &str| !v.is_empty() && v <= wanted;
    match (in_force(current), in_force(candidate)) {
        (false, true) => true,
        (true, false) => false,
        (true, true) => candidate > current,
        (false, false) => current.is_empty() || (!candidate.is_empty() && candidate < current),
    }
}

/// The text of one article of another law, assembled from its corpus entries.
///
/// Returns the text and how many entries the per-article budget left out. The
/// cut lands on an entry boundary rather than mid-sentence: half a lid read as
/// a whole one is a misreading the agent has no way to detect.
fn cited_text(doc: &Value, artikel: &str) -> Option<(String, usize)> {
    let prefix = format!("{artikel}.");
    let empty = Vec::new();
    let parts: Vec<(String, String)> = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty)
        .iter()
        .filter_map(|entry| {
            let number = entry.get("number").and_then(Value::as_str)?;
            if number != artikel && !number.starts_with(&prefix) {
                return None;
            }
            let text = entry.get("text").and_then(Value::as_str)?.trim();
            Some((number.to_owned(), text.to_owned()))
        })
        .collect();
    if parts.is_empty() {
        return None;
    }

    let total = parts.len();
    let mut used = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for (number, text) in parts {
        let line = if number == artikel {
            text
        } else {
            let path = number.strip_prefix(&prefix).unwrap_or(&number);
            format!("[{path}] {text}")
        };
        if !lines.is_empty() && used + line.len() > CROSS_LAW_ARTICLE_CHARS {
            break;
        }
        used += line.len();
        lines.push(line);
    }
    let dropped = total - lines.len();
    Some((lines.join("\n\n"), dropped))
}

/// Resolve every citation the window makes into what the brief can show.
///
/// Order is the order the window cites them in, and the budget is spent in that
/// order, so what falls off the end is what the window mentions last rather
/// than whatever the filesystem happened to yield last.
#[must_use]
pub fn resolve_citations(
    doc: &Value,
    known: &[String],
    window: &[String],
    corpus_root: &Path,
) -> Vec<CitedArticle> {
    let own_bwb = doc
        .get("bwb_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let valid_from = doc
        .get("valid_from")
        .and_then(Value::as_str)
        .unwrap_or_default();

    // Deduplicate on (law, article) while keeping first-mention order, and
    // remember every window article that cites it: a citation shared by both
    // articles of a window is one piece of text, not two.
    let mut order: Vec<(String, String)> = Vec::new();
    let mut by_key: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (bwb, artikel, from) in window_citations(doc, known, window, own_bwb) {
        let key = (bwb, artikel);
        let entry = by_key.entry(key.clone()).or_default();
        if entry.is_empty() {
            order.push(key);
        }
        if !entry.contains(&from) {
            entry.push(from);
        }
    }
    if order.is_empty() {
        return Vec::new();
    }

    let wanted: BTreeSet<String> = order.iter().map(|(bwb, _)| bwb.clone()).collect();
    let located = locate_laws(corpus_root, &wanted, valid_from);
    let mut parsed: BTreeMap<&String, (String, Value)> = BTreeMap::new();

    let mut budget = CROSS_LAW_TOTAL_CHARS;
    let mut out = Vec::new();
    for key in &order {
        let (bwb, artikel) = key;
        let cited_by = by_key.get(key).cloned().unwrap_or_default();
        let Some(path) = located.get(bwb) else {
            out.push(CitedArticle {
                bwb_id: bwb.clone(),
                law_id: String::new(),
                artikel: artikel.clone(),
                cited_by,
                body: CitedBody::NotInCorpus,
            });
            continue;
        };
        if !parsed.contains_key(bwb) {
            match std::fs::read_to_string(path)
                .map_err(|e| format!("cannot be read: {e}"))
                .and_then(|raw| {
                    serde_yaml_ng::from_str::<Value>(&raw)
                        .map_err(|e| format!("is not valid YAML: {e}"))
                }) {
                Ok(doc) => {
                    let law_id = doc
                        .get("$id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    parsed.insert(bwb, (law_id, doc));
                }
                Err(reason) => {
                    out.push(CitedArticle {
                        bwb_id: bwb.clone(),
                        law_id: String::new(),
                        artikel: artikel.clone(),
                        cited_by,
                        body: CitedBody::Unreadable(reason),
                    });
                    continue;
                }
            }
        }
        let Some((law_id, target)) = parsed.get(bwb) else {
            // The parse above either inserted this key or pushed an entry and
            // continued, so this arm is unreachable. Saying so beats a silent
            // skip, which is the failure this whole variant exists to close.
            out.push(CitedArticle {
                bwb_id: bwb.clone(),
                law_id: String::new(),
                artikel: artikel.clone(),
                cited_by,
                body: CitedBody::Unreadable("could not be loaded".to_owned()),
            });
            continue;
        };
        let body = match cited_text(target, artikel) {
            // The law is here and the article is not: a gap in the text, not a
            // defect in the file, and the same advice applies as for a law the
            // corpus does not have at all.
            None => CitedBody::NotInCorpus,
            Some((text, dropped)) if text.len() <= budget => {
                budget -= text.len();
                CitedBody::Text { text, dropped }
            }
            Some(_) => CitedBody::OverBudget,
        };
        out.push(CitedArticle {
            bwb_id: bwb.clone(),
            law_id: law_id.clone(),
            artikel: artikel.clone(),
            cited_by,
            body,
        });
    }
    out
}

/// Whether a definition provision governs an article, and on what evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Governs {
    /// It governs the article, and the document establishes that it does.
    Yes,
    /// It does not govern the article.
    No,
    /// It would govern the article if the two sit in the same container, and
    /// nobody has established where either of them sits.
    Unknown,
}

/// Whether one definition provision governs one article.
///
/// Two independent facts decide this, and the older version used only the
/// second. The first is what the provision says about itself: "in deze wet"
/// reaches everything, "in deze afdeling" reaches one afdeling, "voor de
/// toepassing van dit artikel" reaches nothing else at all. The second is
/// where the two articles sit, which bounds every scope narrower than the
/// whole law.
///
/// Placement comes from the source gate's sidecar and the enrich path runs
/// without it, so the common case is that the second fact is simply not
/// available. That is `Unknown`, not `Yes`: a container-scoped definition
/// whose container nobody knows may well govern this article, and may equally
/// well be the begripsbepaling of a different afdeling.
#[must_use]
pub fn governs(definition: &AssembledArticle, article: &AssembledArticle) -> Governs {
    if definition.number == article.number {
        // An article's own definitions are already in front of the agent.
        return Governs::No;
    }
    let Some(scope) = definition.definition_scope else {
        return Governs::No;
    };
    match scope {
        DefinitionScope::WholeLaw => Governs::Yes,
        DefinitionScope::ArticleOnly => Governs::No,
        _ => match definition.path.encloses(&article.path) {
            None => Governs::Unknown,
            Some(false) => Governs::No,
            // Sharing a prefix is necessary but not sufficient: a definition
            // for "deze afdeling" placed in a hoofdstuk that holds several
            // afdelingen encloses articles in all of them. Require the level
            // the scope names to be present and identical on both sides.
            Some(true) => {
                if same_container_at(&definition.path, &article.path, scope) {
                    Governs::Yes
                } else {
                    Governs::No
                }
            }
        },
    }
}

/// Whether two placements name the same container at the level a scope bounds.
///
/// The path is "Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht", so the level is
/// found by its leading word. A scope that names a level neither placement
/// carries governs nothing it can prove: the provision claims an afdeling and
/// the document knows of none.
fn same_container_at(definition: &Placement, article: &Placement, scope: DefinitionScope) -> bool {
    let Some(prefix) = scope.container_prefix() else {
        return true;
    };
    let segment = |p: &Placement| -> Option<String> {
        let Placement::Known(path) = p else {
            return None;
        };
        path.split(" > ")
            .find(|s| s.starts_with(prefix))
            .map(str::to_owned)
    };
    match (segment(definition), segment(article)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Definition articles that bear on an article, with what is known about each.
///
/// Returns the ones that govern and the ones that might, because the brief has
/// to distinguish them: an agent told that a definition governs an article is
/// being told a fact, and an agent told that placement is unavailable is being
/// told to read the provision's own words and decide.
#[must_use]
pub fn governing_definitions<'a>(
    law: &'a LawContext,
    article: &AssembledArticle,
) -> Vec<(&'a AssembledArticle, Governs)> {
    law.definitions
        .iter()
        .filter_map(|d| match governs(d, article) {
            Governs::No => None,
            verdict => Some((d, verdict)),
        })
        .collect()
}

/// The brief the agent reads before it translates.
///
/// Ordered by what it costs the agent to miss: where the article sits, then
/// what defines its terms, then what bends it. Every section says when it is
/// empty, because a heading with nothing under it reads as "not looked at".
#[must_use]
pub fn render_brief(law: &LawContext, window: &[String], cited: &[CitedArticle]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Context for {} ({}, {})\n\n\
         Written by the worker. You may cite anything in this file. Nothing here\n\
         is a source for a rule: the rule comes from the statutory text of the\n\
         article you are translating.\n",
        law.law_id, law.bwb_id, law.valid_from
    );

    let inbound = inbound_modifiers(law, window);
    let by_target: BTreeMap<&str, Vec<&Inbound>> =
        inbound.iter().fold(BTreeMap::new(), |mut acc, i| {
            acc.entry(i.to.as_str()).or_default().push(i);
            acc
        });

    let per_article = render_definitions(&mut out, law, window);

    for number in window {
        let Some(article) = law.articles.iter().find(|a| &a.number == number) else {
            continue;
        };
        let _ = writeln!(out, "## Article {number}");
        match &article.path {
            Placement::NoContainer => {
                let _ = writeln!(out, "\nPlacement: no enclosing container.");
            }
            Placement::Known(path) => {
                let _ = writeln!(out, "\nPlacement: {path}");
            }
            Placement::Unknown => {
                let _ = writeln!(
                    out,
                    "\nPlacement: not established. Nobody has recorded which containers \n\
                     enclose this article, so this is not a claim that none do."
                );
            }
        }

        match per_article.get(number.as_str()).filter(|i| !i.is_empty()) {
            None => {
                let _ = writeln!(out, "\nNo definition provision governs this article.");
            }
            Some(items) => {
                let _ = writeln!(
                    out,
                    "\nDefinitions that bear on it, from the section above:"
                );
                for (number, verdict) in items {
                    let _ = match verdict {
                        Governs::Unknown => writeln!(
                            out,
                            "\n- Article {number} — only if it sits in the same container as \
                             this one, which is not established."
                        ),
                        _ => writeln!(out, "\n- Article {number}"),
                    };
                }
            }
        }

        match by_target.get(number.as_str()) {
            None => {
                let _ = writeln!(
                    out,
                    "\n### What modifies it\n\nNo other article in this law modifies it."
                );
            }
            Some(items) => {
                let _ = writeln!(
                    out,
                    "\n### What modifies it\n\nThese articles bend this one. Translate the rule \
                     as they leave it, not as this article reads alone."
                );
                for i in items {
                    let _ = writeln!(
                        out,
                        "\n- **Article {}** (\"{}\"): {}",
                        i.from, i.connective, i.sentence
                    );
                }
            }
        }
        out.push('\n');
    }

    render_citations(&mut out, cited);
    out
}

/// The definitions section, written once for the whole window.
///
/// Returns, per window article, the definition articles that bear on it and on
/// what evidence, so the per-article sections can refer to them by number.
///
/// Once, not per article. The brief rides along on every turn of the window,
/// and a definition article runs to kilobytes: the two begripsbepalingen of
/// the Wet langdurige zorg are 7.8 KB together, which a window of ten
/// articles repeated ten times into an 81 KB brief that was almost entirely
/// the same text. The per-article sections lose nothing by naming an article
/// number instead of restating it, because the reader has the text a screen
/// above.
fn render_definitions<'a>(
    out: &mut String,
    law: &'a LawContext,
    window: &'a [String],
) -> BTreeMap<&'a str, Vec<(&'a str, Governs)>> {
    let mut per_article: BTreeMap<&str, Vec<(&str, Governs)>> = BTreeMap::new();
    let mut order: Vec<(&AssembledArticle, Governs)> = Vec::new();
    for number in window {
        let Some(article) = law.articles.iter().find(|a| &a.number == number) else {
            continue;
        };
        for (definition, verdict) in governing_definitions(law, article) {
            per_article
                .entry(article.number.as_str())
                .or_default()
                .push((definition.number.as_str(), verdict));
            match order
                .iter_mut()
                .find(|(d, _)| d.number == definition.number)
            {
                // One article may govern for certain while another only might.
                // The section states the stronger of the two and the
                // per-article lines carry the difference.
                Some((_, current)) if *current == Governs::Unknown => *current = verdict,
                Some(_) => {}
                None => order.push((definition, verdict)),
            }
        }
    }

    let _ = writeln!(out, "## Definitions in this law that bear on the window\n");
    if order.is_empty() {
        let _ = writeln!(
            out,
            "None: no definition provision of this law reaches any article in this window.\n"
        );
        return per_article;
    }
    let _ = writeln!(
        out,
        "Each provision is given once, whole. The per-article sections below name \n\
         the ones that bear on each article rather than repeating them.\n"
    );
    for (definition, verdict) in order {
        let scope = definition
            .definition_scope
            .map_or("scope not stated", DefinitionScope::describe);
        let placement = match &definition.path {
            Placement::Known(path) => path.clone(),
            Placement::NoContainer => "no enclosing container".to_owned(),
            Placement::Unknown => "placement not established".to_owned(),
        };
        let _ = writeln!(
            out,
            "### Article {} — scope: {scope} ({placement})\n",
            definition.number
        );
        if verdict == Governs::Unknown {
            let _ = writeln!(
                out,
                "*This provision bounds itself to a container. Which articles sit in that \n\
                 container is not established here, so read its own words and judge.*\n"
            );
        }
        let _ = writeln!(out, "{}\n", definition.text);
    }
    per_article
}

/// The cross-law section: what the window cites, and what of it is here.
///
/// Written even when the window cites nothing, for the same reason as the
/// other sections: an absent heading reads as "nobody looked". And every
/// omission is named with its cause, because "not in the corpus" and "cut for
/// space" call for opposite responses and neither is "go and search".
fn render_citations(out: &mut String, cited: &[CitedArticle]) {
    let _ = writeln!(out, "## What this window cites from other laws\n");
    if cited.is_empty() {
        let _ = writeln!(
            out,
            "Nothing: no article in this window carries a reference into another law.\n"
        );
        return;
    }

    let _ = writeln!(
        out,
        "Reproduced from the corpus, at the redaction in force on this law's own\n\
         `valid_from`. This is the only copy of those laws you have: there is no\n\
         corpus to search and nothing to fetch, so an article named below as absent\n\
         is absent, and looking for it costs turns and finds nothing.\n"
    );

    let mut omitted: Vec<String> = Vec::new();
    for item in cited {
        let law = if item.law_id.is_empty() {
            item.bwb_id.clone()
        } else {
            format!("{} ({})", item.law_id, item.bwb_id)
        };
        let by = item.cited_by.join(", ");
        match &item.body {
            CitedBody::Text { text, dropped } => {
                let _ = writeln!(
                    out,
                    "### {law}, article {} — cited by article {by}\n\n{text}\n",
                    item.artikel
                );
                if *dropped > 0 {
                    let _ = writeln!(
                        out,
                        "*{dropped} further entr{} of this article left out for space.*\n",
                        if *dropped == 1 { "y" } else { "ies" }
                    );
                }
            }
            CitedBody::NotInCorpus => omitted.push(format!(
                "- {law}, article {}: not in this corpus. Translate what this law says \
                 about it and mark what depends on text you do not have.",
                item.artikel
            )),
            CitedBody::OverBudget => omitted.push(format!(
                "- {law}, article {}: in the corpus, left out because this section's \
                 budget was spent.",
                item.artikel
            )),
            CitedBody::Unreadable(reason) => omitted.push(format!(
                "- {law}, article {}: in this corpus, but it {reason}. Nothing to look \
                 for and nothing to fetch; this is a defect in the corpus.",
                item.artikel
            )),
        }
    }

    if !omitted.is_empty() {
        let _ = writeln!(out, "### Cited but not reproduced\n");
        for line in omitted {
            let _ = writeln!(out, "{line}");
        }
        out.push('\n');
    }
}

/// Assemble the law beside `yaml_abs` and write the brief next to it.
///
/// Returns the path written, or `None` when the file carries no articles.
///
/// The source-context sidecar is optional. Without it the placement lines say
/// so instead of being absent, because "no enclosing container" and "nobody
/// looked" must not read the same. The inbound scan needs no sidecar at all:
/// it reads the article text this file already carries.
pub fn write_brief(
    yaml_abs: &Path,
    window: Option<&[String]>,
    corpus_root: &Path,
) -> Option<PathBuf> {
    let body = std::fs::read_to_string(yaml_abs).ok()?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&body).ok()?;

    let dir = yaml_abs.parent()?;
    let sidecar: Option<super::source_gate::ContextSidecar> =
        std::fs::read_to_string(dir.join(super::source_gate::CONTEXT_SIDECAR))
            .ok()
            .and_then(|s| serde_yaml_ng::from_str(&s).ok());

    // Without the gate's list of real article numbers the split of a corpus
    // entry like `5.2.1` is guesswork, so fall back to the entry numbers
    // themselves: coarser, and never wrong about which article exists.
    let known: Vec<String> = match sidecar.as_ref() {
        Some(s) => s.articles.keys().cloned().collect(),
        None => doc
            .get("articles")
            .and_then(serde_yaml_ng::Value::as_sequence)
            .map(|arts| {
                arts.iter()
                    .filter_map(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    };

    let law = super::assemble::assemble(&doc, &known, sidecar.as_ref());
    if law.articles.is_empty() {
        return None;
    }
    // The window arrives in corpus entry numbers (`1.1.c`), while the brief
    // speaks in article numbers (`1`), because the article is the unit of work
    // and the entry is a fragment of one. Mapping the window through the same
    // split the assembler uses is what keeps the two from silently missing
    // each other: an unmapped window matches nothing and yields a brief with
    // a heading and no content.
    let scope: Vec<String> = match window {
        Some(w) => {
            let mut mapped: Vec<String> = w
                .iter()
                .map(|entry| super::assemble::split_number(&known, entry).0.to_owned())
                .filter(|a| !a.is_empty())
                .collect();
            mapped.sort();
            mapped.dedup();
            mapped
        }
        None => law.articles.iter().map(|a| a.number.clone()).collect(),
    };
    let scope = if scope.is_empty() {
        law.articles.iter().map(|a| a.number.clone()).collect()
    } else {
        scope
    };

    let cited = resolve_citations(&doc, &known, &scope, corpus_root);

    let path = dir.join(CONTEXT_BRIEF);
    std::fs::write(&path, render_brief(&law, &scope, &cited)).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An article with placement established: the path, or no container.
    fn article(number: &str, path: &str, text: &str) -> AssembledArticle {
        let mut a = unplaced(number, text);
        a.path = if path.is_empty() {
            Placement::NoContainer
        } else {
            Placement::Known(path.to_owned())
        };
        a
    }

    /// An article nobody has established the placement of, which is what the
    /// enrich path sees whenever the source gate has not run.
    fn unplaced(number: &str, text: &str) -> AssembledArticle {
        // The scope comes out of the text by the same route production uses,
        // so a test cannot assert a scope the words do not carry.
        let doc: Value = serde_yaml_ng::from_str(&format!(
            "articles:\n  - number: {number:?}\n    text: {text:?}\n"
        ))
        .expect("parse");
        let assembled = super::super::assemble::assemble(&doc, &[number.to_owned()], None);
        assembled.articles.into_iter().next().expect("one article")
    }

    fn law(articles: Vec<AssembledArticle>, definitions: Vec<AssembledArticle>) -> LawContext {
        LawContext {
            law_id: "test_wet".to_owned(),
            bwb_id: "BWBR0000000".to_owned(),
            valid_from: "2025-01-01".to_owned(),
            definitions,
            articles,
        }
    }

    #[test]
    fn finds_a_plain_reference() {
        assert_eq!(referenced_articles("bedoeld in artikel 8"), vec!["8"]);
    }

    #[test]
    fn finds_an_enumeration() {
        assert_eq!(
            referenced_articles("de artikelen 8 en 9 zijn van toepassing"),
            vec!["8", "9"]
        );
    }

    #[test]
    fn keeps_the_article_and_drops_the_lid() {
        // The node is the article; the lid is an address decided elsewhere.
        assert_eq!(
            referenced_articles("in afwijking van artikel 8, tweede lid"),
            vec!["8"]
        );
    }

    #[test]
    fn handles_letter_suffixed_numbering() {
        assert_eq!(referenced_articles("artikel 3a bepaalt"), vec!["3a"]);
    }

    #[test]
    fn modifying_reference_from_outside_the_window_is_found() {
        // The failure this module exists for: the influence runs from 10 to 8
        // while the reference points from 10 at 8.
        let l = law(
            vec![
                article("8", "", "De toeslag bedraagt het standaardbedrag."),
                article(
                    "10",
                    "",
                    "In afwijking van artikel 8 bedraagt de toeslag nihil bij een vermogen boven de grens.",
                ),
            ],
            vec![],
        );
        let found = inbound_modifiers(&l, &["8".to_owned()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].from, "10");
        assert_eq!(found[0].to, "8");
        assert_eq!(found[0].connective, "in afwijking van");
    }

    #[test]
    fn merely_using_an_article_is_not_a_modification() {
        // "Het bedrag, bedoeld in artikel 8" uses article 8 and leaves it be.
        let l = law(
            vec![
                article("8", "", "De toeslag bedraagt het standaardbedrag."),
                article(
                    "12",
                    "",
                    "Het bedrag, bedoeld in artikel 8, wordt jaarlijks bekendgemaakt.",
                ),
            ],
            vec![],
        );
        assert!(inbound_modifiers(&l, &["8".to_owned()]).is_empty());
    }

    #[test]
    fn an_article_does_not_modify_itself() {
        let l = law(
            vec![article(
                "8",
                "",
                "In afwijking van artikel 8, eerste lid, geldt het tweede lid.",
            )],
            vec![],
        );
        assert!(inbound_modifiers(&l, &["8".to_owned()]).is_empty());
    }

    #[test]
    fn a_url_fragment_is_not_a_reference() {
        // Measured on the Awir in round 3: the harvested text carries markdown
        // reference footers, and `#Artikel1` inside a URL made article 1 look
        // like the target of two modifications it does not have.
        assert!(referenced_articles("zie https://wetten.nl/BWBR0008659#Artikel1").is_empty());
    }

    #[test]
    fn reference_footers_are_not_statutory_text() {
        // The footer holds full stops inside a URL, so splitting on sentences
        // cuts through it and the halves get quoted as if they were the law.
        let text = "In afwijking van artikel 8 geldt nihil.\n\n[ref1]: https://wetten.nl/BWBR0008659#Artikel1\n";
        let found = sentences(text);
        assert!(
            found.iter().all(|s| !s.contains("wetten.nl")),
            "got: {found:?}"
        );
    }

    #[test]
    fn a_reference_into_another_law_is_not_an_inbound_modifier() {
        // The brief is about what one law does to itself. A relation to some
        // other statute cannot be checked against the file in front of the
        // reader, and cross-law edges belong to the work queue.
        let l = law(
            vec![
                article("1", "", "Begripsbepalingen."),
                article(
                    "31ter",
                    "",
                    "In afwijking van artikel 31bis geldt dit voor wie recht heeft op een tegemoetkoming als bedoeld in artikel 1 van die wet.",
                ),
            ],
            vec![],
        );
        assert!(
            inbound_modifiers(&l, &["1".to_owned()]).is_empty(),
            "article 1 of another law was read as article 1 of this one"
        );
    }

    #[test]
    fn a_lid_qualifier_does_not_hide_the_other_law() {
        assert!(is_external_reference(
            " 8, tweede lid, van de Zorgverzekeringswet"
        ));
        assert!(is_external_reference(" 8 van de Awir"));
        assert!(!is_external_reference(" 8 geldt niet"));
        assert!(!is_external_reference(" 8 van deze wet"));
    }

    #[test]
    fn definitions_are_scoped_by_container() {
        let l = law(
            vec![article(
                "8",
                "Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht",
                "tekst",
            )],
            vec![
                article("1", "", "In deze wet wordt verstaan onder: a. toeslag: ..."),
                article(
                    "7",
                    "Hoofdstuk 3 Toeslagen",
                    "In dit hoofdstuk wordt verstaan onder: b. peildatum: ...",
                ),
                article(
                    "20",
                    "Hoofdstuk 4 Bezwaar",
                    "In dit hoofdstuk wordt verstaan onder: c. termijn: ...",
                ),
            ],
        );
        let governing: Vec<&str> = governing_definitions(&l, &l.articles[0])
            .iter()
            .map(|(d, _)| d.number.as_str())
            .collect();
        // The law-wide one and the enclosing chapter's, never the sibling
        // chapter's.
        assert_eq!(governing, vec!["1", "7"]);
    }

    #[test]
    fn a_division_scoped_definition_does_not_govern_the_whole_law() {
        // Measured on the Wet basisregistratie personen: article 1.1 says "in
        // deze wet" and article 2.62 says "in deze afdeling", and the brief
        // presented 2.62 to 1.1 as governing the whole law.
        let l = law(
            vec![article(
                "1.1",
                "Hoofdstuk 1 Algemeen",
                "De minister houdt een register bij.",
            )],
            vec![
                article(
                    "1.1",
                    "Hoofdstuk 1 Algemeen",
                    "In deze wet wordt verstaan onder: a. ingeschrevene: ...",
                ),
                article(
                    "2.62",
                    "Hoofdstuk 2 Basisregistratie > Afdeling 3 Aangewezen bestuursorganen",
                    "In deze afdeling wordt verstaan onder een aangewezen bestuursorgaan: ...",
                ),
            ],
        );
        let governing: Vec<&str> = governing_definitions(&l, &l.articles[0])
            .iter()
            .map(|(d, _)| d.number.as_str())
            .collect();
        assert!(
            !governing.contains(&"2.62"),
            "a definition for one afdeling was offered to an article outside it"
        );
    }

    #[test]
    fn an_article_scoped_definition_governs_nothing_but_itself() {
        let l = law(
            vec![article("8", "", "De toeslag bedraagt X.")],
            vec![article(
                "7",
                "",
                "Voor de toepassing van dit artikel wordt verstaan onder: a. jaar: ...",
            )],
        );
        assert!(governing_definitions(&l, &l.articles[0]).is_empty());
    }

    #[test]
    fn a_definition_that_states_no_scope_is_not_read_as_law_wide() {
        // "wordt verstaan onder" with no scope phrase claims nothing beyond
        // the article it stands in, and reading it law-wide is an assertion
        // the provision never made.
        let l = law(
            vec![article("8", "", "De toeslag bedraagt X.")],
            vec![article(
                "7",
                "",
                "Onder kind wordt verstaan: een eigen kind.",
            )],
        );
        assert!(governing_definitions(&l, &l.articles[0]).is_empty());
    }

    #[test]
    fn unknown_placement_is_neither_governing_nor_absent() {
        // The enrich path runs without the source gate's sidecar, so this is
        // the common case rather than an edge one. A container-scoped
        // definition whose container nobody knows may govern this article and
        // may equally be the begripsbepaling of another afdeling.
        let definition = unplaced(
            "2.62",
            "In deze afdeling wordt verstaan onder: a. orgaan: ...",
        );
        let target = unplaced("1.1", "De minister houdt een register bij.");
        assert_eq!(definition.path, Placement::Unknown);
        assert_eq!(governs(&definition, &target), Governs::Unknown);

        let l = law(vec![target], vec![definition]);
        let brief = render_brief(&l, &["1.1".to_owned()], &[]);
        assert!(
            !brief.contains("whole law"),
            "an unknown scope was presented as law-wide:\n{brief}"
        );
        assert!(brief.contains("not established"), "{brief}");
    }

    #[test]
    fn placement_not_established_does_not_read_as_no_container() {
        let l = law(vec![unplaced("8", "De toeslag bedraagt X.")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &[]);
        assert!(brief.contains("Placement: not established"), "{brief}");
        assert!(!brief.contains("no enclosing container"), "{brief}");
    }

    #[test]
    fn a_shared_definition_is_written_once_for_the_whole_window() {
        // The brief rides along on every turn, and a begripsbepaling runs to
        // kilobytes. Measured on the Wet langdurige zorg, repeating two of
        // them per article turned a ten-article window into 81 KB of almost
        // entirely the same text.
        let body = "In deze wet wordt verstaan onder: a. zorgkantoor: DISTINCTIVE-DEFINITION-BODY.";
        let l = law(
            vec![
                article("8", "Hoofdstuk 3", "Artikel acht."),
                article("9", "Hoofdstuk 3", "Artikel negen."),
                article("10", "Hoofdstuk 3", "Artikel tien."),
            ],
            vec![article("1", "Hoofdstuk 1", body)],
        );
        let brief = render_brief(&l, &["8".to_owned(), "9".to_owned(), "10".to_owned()], &[]);
        assert_eq!(
            brief.matches("DISTINCTIVE-DEFINITION-BODY").count(),
            1,
            "definition text repeated per article:\n{brief}"
        );
        // And every article still knows which provision bears on it.
        assert_eq!(brief.matches("- Article 1").count(), 3, "{brief}");
    }

    #[test]
    fn a_window_growing_does_not_grow_the_definitions_section() {
        let body = "In deze wet wordt verstaan onder: ".to_owned() + &"begrip. ".repeat(500);
        let definitions = vec![article("1", "", &body)];
        let articles: Vec<AssembledArticle> = (1..=10)
            .map(|i| article(&format!("{}", i + 10), "", "Korte tekst."))
            .collect();
        let numbers: Vec<String> = articles.iter().map(|a| a.number.clone()).collect();
        let l = law(articles, definitions);

        let one = render_brief(&l, &numbers[..1], &[]).len();
        let five = render_brief(&l, &numbers[..5], &[]).len();
        let ten = render_brief(&l, &numbers, &[]).len();
        // Ten articles cost ten short sections over one article, not ten
        // copies of a 4 KB provision.
        assert!(
            ten - one < body.len(),
            "brief grew by {} over nine more articles, definition is {} chars",
            ten - one,
            body.len()
        );
        assert!(five > one && ten > five, "{one} {five} {ten}");
    }

    #[test]
    fn brief_names_an_empty_section_rather_than_omitting_it() {
        // A missing heading reads as "not looked at", which is the ambiguity
        // this whole flow is trying to remove.
        let l = law(vec![article("8", "", "De toeslag bedraagt nihil.")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &[]);
        assert!(brief.contains("No definition provision governs this article."));
        assert!(brief.contains("No other article in this law modifies it."));
    }

    #[test]
    fn window_in_entry_numbers_still_finds_its_articles() {
        // The corpus splits below article level, so a window is a list of
        // entries like `1.1.c` while the brief speaks in article numbers. When
        // those two miss each other the brief comes out as a heading with
        // nothing under it, and nothing fails: exactly the silence this flow
        // is built to remove.
        let dir = tempfile::tempdir().expect("tempdir");
        let law = dir.path().join("2026-01-01.yaml");
        std::fs::write(
            &law,
            r#"$schema: https://regelrecht.rijks.app/schema/v0.7.0/schema.json
articles:
  - number: "1.1.a"
    text: "In deze wet wordt verstaan onder: a. toeslag: een tegemoetkoming."
  - number: "8.1"
    text: "Het toetsingsinkomen is het verzamelinkomen."
  - number: "10.1"
    text: "In afwijking van artikel 8 blijft het inkomen buiten beschouwing."
"#,
        )
        .expect("write law");

        // The source gate runs first and leaves the real article numbers
        // behind; without it the split of `8.1` is guesswork.
        std::fs::write(
            dir.path().join(super::super::source_gate::CONTEXT_SIDECAR),
            r#"bwb_id: BWBR0000000
valid_from: "2026-01-01"
articles:
  "1":
    verdict: verified
  "8":
    path: "Hoofdstuk 2 Toeslag"
    verdict: verified
  "10":
    path: "Hoofdstuk 2 Toeslag"
    verdict: verified
"#,
        )
        .expect("write sidecar");

        let written =
            write_brief(&law, Some(&["8.1".to_owned()]), dir.path()).expect("brief written");
        let brief = std::fs::read_to_string(written).expect("read brief");

        assert!(brief.contains("## Article 8"), "brief was: {brief}");
        assert!(
            brief.contains("**Article 10**"),
            "inbound modifier missing from:\n{brief}"
        );
    }

    /// A corpus of two laws: the one being translated and one it cites.
    ///
    /// Shaped like the real thing — `<root>/regulation/nl/wet/<id>/<date>.yaml`
    /// with the cited article split into a lid and two onderdelen — because
    /// the fragment split is precisely what makes a citation hard to look up
    /// by hand and easy to get wrong.
    fn citing_corpus(other_valid_from: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let own = dir.path().join("regulation/nl/wet/wet_toeslag");
        std::fs::create_dir_all(&own).expect("mkdir");
        std::fs::write(
            own.join("2026-01-01.yaml"),
            r#"$id: wet_toeslag
bwb_id: BWBR0000001
valid_from: "2026-01-01"
articles:
  - number: "8.1"
    text: "De verzekerde heeft aanspraak."
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: "1"
      - id: ref2
        bwb_id: BWBR0000002
        artikel: "24"
      - id: ref3
        bwb_id: BWBR0009999
        artikel: "3"
      - id: ref4
        bwb_id: BWBR0000001
        artikel: "9"
  - number: "9.1"
    text: "Buiten het venster."
    references:
      - id: ref5
        bwb_id: BWBR0000002
        artikel: "70"
"#,
        )
        .expect("write own law");

        let other = dir.path().join("regulation/nl/wet/andere_wet");
        std::fs::create_dir_all(&other).expect("mkdir");
        for version in other_valid_from {
            std::fs::write(
                other.join(format!("{version}.yaml")),
                format!(
                    r#"$id: andere_wet
bwb_id: BWBR0000002
valid_from: "{version}"
articles:
  - number: "1"
    text: "Chapeau van artikel 1, redactie {version}:"
  - number: "1.a"
    text: "onderdeel a van artikel 1;"
  - number: "1.b"
    text: "onderdeel b van artikel 1."
  - number: "10"
    text: "Artikel 10 is niet artikel 1."
  - number: "24"
    text: "Artikel 24 zegt iets anders."
  - number: "70"
    text: "Artikel 70 wordt buiten het venster aangehaald."
"#
                ),
            )
            .expect("write other law");
        }
        dir
    }

    fn law_doc(dir: &tempfile::TempDir) -> Value {
        let raw = std::fs::read_to_string(
            dir.path()
                .join("regulation/nl/wet/wet_toeslag/2026-01-01.yaml"),
        )
        .expect("read own law");
        serde_yaml_ng::from_str(&raw).expect("parse own law")
    }

    #[test]
    fn only_the_windows_own_citations_into_other_laws_are_collected() {
        // Three things at once, and each of them cost turns in the measured
        // run: an entry outside the window contributes nothing, a reference
        // back into this same law is not a citation, and the entry number
        // `8.1` has to map onto article 8 for any of it to match.
        let dir = citing_corpus(&["2026-01-01"]);
        let doc = law_doc(&dir);
        let known = vec!["8".to_owned(), "9".to_owned()];
        let found = window_citations(&doc, &known, &["8".to_owned()], "BWBR0000001");
        assert_eq!(
            found,
            vec![
                ("BWBR0000002".to_owned(), "1".to_owned(), "8".to_owned()),
                ("BWBR0000002".to_owned(), "24".to_owned(), "8".to_owned()),
                ("BWBR0009999".to_owned(), "3".to_owned(), "8".to_owned()),
            ]
        );
    }

    #[test]
    fn a_cited_article_arrives_whole_and_a_missing_law_says_so() {
        let dir = citing_corpus(&["2026-01-01"]);
        let doc = law_doc(&dir);
        let known = vec!["8".to_owned(), "9".to_owned()];
        let cited = resolve_citations(&doc, &known, &["8".to_owned()], dir.path());
        assert_eq!(cited.len(), 3);

        // The article is assembled from its lid and its onderdelen, and
        // article 10 does not come along for the ride on a prefix match.
        let first = &cited[0];
        assert_eq!(first.law_id, "andere_wet");
        assert_eq!(first.artikel, "1");
        assert_eq!(first.cited_by, vec!["8".to_owned()]);
        let CitedBody::Text { text, dropped } = &first.body else {
            panic!("expected text, got {:?}", first.body);
        };
        assert_eq!(*dropped, 0);
        assert!(text.contains("Chapeau van artikel 1"), "{text}");
        assert!(text.contains("[a] onderdeel a van artikel 1;"), "{text}");
        assert!(!text.contains("Artikel 10"), "{text}");

        // A law the corpus does not have cannot be found by looking either,
        // which is the thing the agent has to be told rather than left to
        // discover with a grep.
        assert_eq!(cited[2].bwb_id, "BWBR0009999");
        assert_eq!(cited[2].body, CitedBody::NotInCorpus);
        assert!(cited[2].law_id.is_empty());
    }

    #[test]
    fn the_redaction_in_force_wins_over_a_later_one() {
        // An agent shown a different redaction than the one it is translating
        // against is worse off than one shown nothing: it cannot see that the
        // text moved.
        let dir = citing_corpus(&["2020-01-01", "2026-01-01", "2030-01-01"]);
        let doc = law_doc(&dir);
        let known = vec!["8".to_owned(), "9".to_owned()];
        let cited = resolve_citations(&doc, &known, &["8".to_owned()], dir.path());
        let CitedBody::Text { text, .. } = &cited[0].body else {
            panic!("expected text");
        };
        assert!(text.contains("redactie 2026-01-01"), "{text}");
    }

    #[test]
    fn a_long_article_is_cut_on_an_entry_boundary_and_says_how_much() {
        // Half a lid read as a whole one is a misreading with no tell, so the
        // cut lands between entries and the count of what fell off is stated.
        let doc: Value = serde_yaml_ng::from_str(&format!(
            "articles:\n  - number: '1'\n    text: '{}'\n  - number: '1.a'\n    text: 'staart'\n",
            "x".repeat(CROSS_LAW_ARTICLE_CHARS)
        ))
        .expect("parse");
        let (text, dropped) = cited_text(&doc, "1").expect("article 1");
        assert_eq!(dropped, 1);
        assert!(!text.contains("staart"));
    }

    #[test]
    fn what_the_budget_cut_is_named_rather_than_silently_absent() {
        let cited = vec![
            CitedArticle {
                bwb_id: "BWBR0000002".to_owned(),
                law_id: "andere_wet".to_owned(),
                artikel: "1".to_owned(),
                cited_by: vec!["8".to_owned()],
                body: CitedBody::Text {
                    text: "de tekst".to_owned(),
                    dropped: 3,
                },
            },
            CitedArticle {
                bwb_id: "BWBR0000003".to_owned(),
                law_id: String::new(),
                artikel: "5".to_owned(),
                cited_by: vec!["8".to_owned()],
                body: CitedBody::NotInCorpus,
            },
            CitedArticle {
                bwb_id: "BWBR0000004".to_owned(),
                law_id: "derde_wet".to_owned(),
                artikel: "7".to_owned(),
                cited_by: vec!["8".to_owned()],
                body: CitedBody::OverBudget,
            },
        ];
        let l = law(vec![article("8", "", "tekst")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &cited);

        assert!(
            brief.contains("andere_wet (BWBR0000002), article 1"),
            "{brief}"
        );
        assert!(brief.contains("3 further entries"), "{brief}");
        assert!(
            brief.contains("BWBR0000003, article 5: not in this corpus"),
            "{brief}"
        );
        assert!(brief.contains("budget was spent"), "{brief}");
        // And the standing instruction that makes the section worth its size:
        // there is nothing to go and look for.
        assert!(brief.contains("nothing to fetch"), "{brief}");
    }

    #[test]
    fn a_window_that_cites_nothing_still_says_so() {
        // Same rule as every other section: a missing heading reads as "not
        // looked at", and an agent that suspects the worker skipped a step
        // goes and does it by hand.
        let l = law(vec![article("8", "", "tekst")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &[]);
        assert!(brief.contains("What this window cites from other laws"));
        assert!(brief.contains("no article in this window carries a reference"));
    }

    #[test]
    fn brief_says_it_is_not_a_source_for_rules() {
        let l = law(vec![article("8", "", "tekst")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &[]);
        assert!(brief.contains("Nothing here"));
        assert!(brief.contains("statutory text"));
    }

    #[test]
    fn a_cited_law_that_does_not_parse_is_named_rather_than_dropped() {
        // Three arms used to `continue` the outer loop on a read or parse
        // failure, so the law appeared neither in the brief nor under "Cited
        // but not reproduced" — under a heading that promises the list is
        // exhaustive.
        let dir = citing_corpus(&["2026-01-01"]);
        let broken = dir
            .path()
            .join("regulation/nl/wet/andere_wet/2026-01-01.yaml");
        std::fs::write(
            &broken,
            "$id: andere_wet\nbwb_id: BWBR0000002\narticles: [ unclosed\n",
        )
        .expect("write broken law");

        let doc = law_doc(&dir);
        let known = vec!["8".to_owned(), "9".to_owned()];
        let cited = resolve_citations(&doc, &known, &["8".to_owned()], dir.path());

        // Both citations into the broken law are still accounted for.
        let broken: Vec<&CitedArticle> =
            cited.iter().filter(|c| c.bwb_id == "BWBR0000002").collect();
        assert_eq!(broken.len(), 2, "citations vanished: {cited:?}");
        for item in &broken {
            assert!(
                matches!(item.body, CitedBody::Unreadable(_)),
                "expected Unreadable, got {:?}",
                item.body
            );
        }

        let l = law(vec![article("8", "", "tekst")], vec![]);
        let brief = render_brief(&l, &["8".to_owned()], &cited);
        assert!(brief.contains("Cited but not reproduced"), "{brief}");
        assert!(
            brief.contains("BWBR0000002, article 1: in this corpus, but it is not valid YAML"),
            "{brief}"
        );
        assert!(brief.contains("defect in the corpus"), "{brief}");
    }

    #[test]
    fn brief_carries_placement_and_the_modifier() {
        let l = law(
            vec![
                article(
                    "8",
                    "Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht",
                    "De toeslag bedraagt X.",
                ),
                article(
                    "10",
                    "Hoofdstuk 3 Toeslagen",
                    "In afwijking van artikel 8 geldt nihil.",
                ),
            ],
            vec![],
        );
        let brief = render_brief(&l, &["8".to_owned()], &[]);
        assert!(brief.contains("Placement: Hoofdstuk 3 Toeslagen > Afdeling 3.1 Recht"));
        assert!(brief.contains("**Article 10**"));
        assert!(brief.contains("in afwijking van"));
    }
}
