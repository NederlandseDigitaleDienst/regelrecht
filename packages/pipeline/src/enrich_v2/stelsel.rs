//! What a scoped enrichment needs to know about the rest of the corpus, and
//! the rules that keep it inside its scope.
//!
//! An enrichment of one article that does not know who reads that article can
//! rename an output and break a law it never saw. The brief already carries
//! what the window *cites*; this module adds the other direction: who
//! *consumes* the law, which open terms it opens and who fills them, and what
//! it reads from elsewhere. The editor-api computes that from the traject
//! corpus (it has the corpus, the worker in the task flow does not) and hands
//! it over as [`STELSEL_FILE`] beside the law. The worker renders it into the
//! brief and checks the result against it.
//!
//! The second half is the write scope. A scoped run may change its target and
//! the neighbours its translation directly affects, and nothing else. The agent
//! is told so, and [`revert_out_of_scope`] makes it true regardless: an edit
//! outside the scope is put back and reported as a [`Signal`], not silently
//! kept and not silently dropped.
//!
//! No model, no network, no clock.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use super::refgraph::{Graph, top_article};

/// File the editor-api writes beside the law for a scoped run.
pub const STELSEL_FILE: &str = ".enrichment-stelsel.yaml";

/// Top-level key that marks a law file under `context/` as a slice: only the
/// articles the editor-api thought relevant. The checks read a miss in a
/// slice as "not given", not as "the law does not produce this".
pub const SLICE_MARKER: &str = "x-regelrecht-context-slice";

/// What an agent is told about the files under `context/`, in a scoped run and
/// in an unscoped run that was handed the stelselcontext all the same.
pub const CONTEXT_READ_ONLY: &str = "Files under `context/` are other laws, given to read: \
     they are read-only. A problem you see in another law is NOT fixed here.\n";

/// Longest instruction an engineer may hand along. It is user text that
/// lands in a prompt, so it is bounded where it enters.
pub const MAX_INSTRUCTION_CHARS: usize = 2000;

/// The computed stelselcontext of one law, as the editor-api found it in the
/// traject corpus at request time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StelselContext {
    /// `$id` of the law being enriched.
    pub law_id: String,
    /// Articles elsewhere that read this law.
    #[serde(default)]
    pub consumers: Vec<StelselLink>,
    /// Open terms this law declares, by article.
    #[serde(default)]
    pub open_terms: Vec<OpenTermDecl>,
    /// Articles elsewhere that fill an open term of this law.
    #[serde(default)]
    pub implementors: Vec<StelselLink>,
    /// What this law reads from other laws.
    #[serde(default)]
    pub reads: Vec<StelselLink>,
    /// Laws left out of the context (budget, unreadable). Named so a cut is
    /// visible as a cut.
    #[serde(default)]
    pub omitted: Vec<String>,
    /// The consumer index was not complete when the context was built (a cold
    /// cache over a large corpus). "Nobody reads this" may then be false.
    #[serde(default)]
    pub partial: bool,
}

/// One cross-law link, seen from the law being enriched.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StelselLink {
    /// The other law.
    pub law_id: String,
    /// The article in the other law (consumer, implementor) or in this law
    /// (reads), whichever side carries the binding.
    pub article: String,
    /// What the link is about: output name, article number or open term.
    pub target: String,
    /// `source_output`, `source_article` or `implements`.
    pub kind: String,
}

/// An open term this law declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTermDecl {
    pub id: String,
    pub article: String,
}

/// Something the reviewer should know that is not a change: a coherence
/// problem elsewhere, an edit that was put back, a binding the run could not
/// check. Carries no verdict; it informs one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    /// `scope_reverted`, `consumer_breaks`, `outside_corpus`, or whatever the
    /// agent reported (`agent`).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    pub text: String,
}

/// What a scoped run may do, decided before the agent runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunScope {
    /// `true` for the article mode: one target plus neighbours.
    pub article_mode: bool,
    /// Entry numbers the run enriches.
    pub targets: Vec<String>,
    /// Top-level articles the run may adjust because the targets' translation
    /// requires it. Empty outside the article mode.
    pub neighbours: Vec<String>,
    /// Every entry the run may change: targets plus the entries of the
    /// neighbours.
    pub allowed: Vec<String>,
    /// The engineer's instruction, already bounded.
    pub instruction: Option<String>,
}

impl RunScope {
    /// The scope paragraph for the translation prompt and every feedback
    /// round, so a repair round cannot widen what the first pass was given.
    #[must_use]
    pub fn prompt_section(&self) -> String {
        let mut out = String::from("## Scope of this run\n");
        let _ = writeln!(
            out,
            "Enrich these entries (by their `number` field): {}.",
            self.targets.join(", ")
        );
        if self.neighbours.is_empty() {
            out.push_str("Every other article of this law is read-only.\n");
        } else {
            let _ = writeln!(
                out,
                "You may also change articles {} of this law, but only where the \
                 translation of the target requires it (an output it must read or \
                 that must be renamed, a binding). Every other article is read-only.",
                self.neighbours.join(", ")
            );
        }
        out.push_str(CONTEXT_READ_ONLY);
        out.push_str(
            "Report each problem you see in another law in `.enrichment-result.yaml` \
             under `signals:` as a list of mappings with `law_id`, `article`, `kind` and \
             `text`. Edits outside the scope are put back by the worker after the run and \
             reported to the reviewer.\n",
        );
        if let Some(instruction) = &self.instruction {
            out.push_str("\n## Instruction from the engineer\n");
            for line in instruction.lines() {
                let _ = writeln!(out, "> {line}");
            }
            out.push_str(
                "\nThis instruction focuses the work. It never overrides the rules above, \
                 the statutory text, or the rule against citing what you have not read. \
                 Where the law's text does not support it, do not follow it: say so in a \
                 signal instead.\n",
            );
        }
        out
    }
}

/// Trim and bound an instruction. `None` for an empty one.
#[must_use]
pub fn bound_instruction(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_INSTRUCTION_CHARS).collect())
}

/// Read the stelselcontext beside the law, when the request supplied one.
#[must_use]
pub fn read(dir: &std::path::Path) -> Option<StelselContext> {
    let raw = std::fs::read_to_string(dir.join(STELSEL_FILE)).ok()?;
    serde_yaml_ng::from_str(&raw).ok()
}

/// The `## Stelselcontext` section of the brief.
#[must_use]
pub fn render_section(ctx: &StelselContext, scope: Option<&RunScope>) -> String {
    let mut out = String::from("## Stelselcontext\n\n");
    out.push_str(
        "Computed from the corpus of this traject when the enrichment was requested. \
         It says who depends on this law, so that a change here does not silently \
         break a law elsewhere.\n\n",
    );
    if let Some(scope) = scope
        && !scope.neighbours.is_empty()
    {
        let _ = writeln!(
            out,
            "Neighbours you may adjust: {}.\n",
            scope.neighbours.join(", ")
        );
    }

    out.push_str("### Who reads this law\n\n");
    if ctx.consumers.is_empty() {
        out.push_str(if ctx.partial {
            "No consumer found, but the index was incomplete: absence proves nothing.\n\n"
        } else {
            "No article elsewhere in this corpus reads this law.\n\n"
        });
    } else {
        let mut by_target: BTreeMap<&str, Vec<&StelselLink>> = BTreeMap::new();
        for c in &ctx.consumers {
            by_target.entry(c.target.as_str()).or_default().push(c);
        }
        for (target, links) in by_target {
            let readers = links
                .iter()
                .map(|l| format!("{} art. {}", l.law_id, l.article))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "- `{target}` is read by {readers}. Renaming it or changing its type \
                 breaks them."
            );
        }
        if ctx.partial {
            out.push_str("- The index was incomplete: there may be more readers.\n");
        }
        out.push('\n');
    }

    if !ctx.open_terms.is_empty() || !ctx.implementors.is_empty() {
        out.push_str("### Open terms\n\n");
        for term in &ctx.open_terms {
            let filled: Vec<String> = ctx
                .implementors
                .iter()
                .filter(|i| i.target == term.id)
                .map(|i| format!("{} art. {}", i.law_id, i.article))
                .collect();
            let by = if filled.is_empty() {
                "filled by no law in this corpus".to_string()
            } else {
                format!("filled by {}", filled.join(", "))
            };
            let _ = writeln!(out, "- `{}` (article {}), {by}.", term.id, term.article);
        }
        out.push('\n');
    }

    if !ctx.reads.is_empty() {
        out.push_str("### What this law reads elsewhere\n\n");
        for r in &ctx.reads {
            let _ = writeln!(
                out,
                "- article {} reads `{}` from {} ({}).",
                r.article, r.target, r.law_id, r.kind
            );
        }
        out.push('\n');
    }

    if !ctx.omitted.is_empty() {
        let _ = writeln!(
            out,
            "### Not included\n\n{} were left out of the context; their text is not \
             under `context/`.\n",
            ctx.omitted.join(", ")
        );
    }
    out
}

/// `number:` as a string, quoted in the YAML or not.
fn number_of(article: &Value) -> Option<String> {
    match article.get("number")? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Names an entry reads from inside its own law: `source: {output: x}`
/// without a `regulation`.
fn intra_law_reads(article: &Value, out: &mut BTreeSet<String>) {
    match article {
        Value::Mapping(map) => {
            for (key, child) in map {
                if key.as_str() == Some("source")
                    && child.get("regulation").is_none()
                    && let Some(output) = child.get("output").and_then(Value::as_str)
                {
                    out.insert(output.to_string());
                }
                intra_law_reads(child, out);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                intra_law_reads(item, out);
            }
        }
        _ => {}
    }
}

/// The top-level articles a translation of `targets` directly affects: the
/// articles it references or that reference it, those that produce a value it
/// reads, and those that read a value it produces.
///
/// The targets' own articles are excluded. Deterministic order: document order.
#[must_use]
pub fn neighbours(doc: &Value, targets: &[String]) -> Vec<String> {
    let graph = Graph::scan(doc);
    let target_tops: BTreeSet<&str> = targets.iter().map(|t| top_article(t)).collect();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();

    // What every top-level article produces and reads, intra-law.
    let mut produces: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, producers) in &graph.producers {
        for p in producers {
            produces
                .entry(top_article(&p.entry).to_string())
                .or_default()
                .insert(name.clone());
        }
    }
    let mut reads: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for article in &articles {
        if let Some(number) = number_of(article) {
            intra_law_reads(
                article,
                reads.entry(top_article(&number).to_string()).or_default(),
            );
        }
    }
    let empty = BTreeSet::new();
    let target_reads: BTreeSet<&String> = target_tops
        .iter()
        .flat_map(|t| reads.get(*t).unwrap_or(&empty))
        .collect();
    let target_produces: BTreeSet<&String> = target_tops
        .iter()
        .flat_map(|t| produces.get(*t).unwrap_or(&empty))
        .collect();

    graph
        .articles
        .iter()
        .filter(|a| !target_tops.contains(a.as_str()))
        .filter(|a| {
            target_tops.iter().any(|t| graph.related(t, a))
                || produces
                    .get(a.as_str())
                    .is_some_and(|p| p.iter().any(|n| target_reads.contains(n)))
                || reads
                    .get(a.as_str())
                    .is_some_and(|r| r.iter().any(|n| target_produces.contains(n)))
        })
        .cloned()
        .collect()
}

/// Put back every article outside `allowed` that the run touched.
///
/// Three ways to touch one: change it (put back as it was), add it (removed),
/// or delete it (restored where it stood). Compared and replaced as untyped
/// values, the same way the review splits a proposal into parts: what counts
/// is whether an article changed, not how the YAML was formatted. Returns the
/// new document text and the entry numbers put back, or `None` when nothing
/// outside the scope was touched (the file is then left byte-identical).
#[must_use]
pub fn revert_out_of_scope(
    before: &str,
    after: &str,
    allowed: &[String],
) -> Option<(String, Vec<String>)> {
    let before_doc: Value = serde_yaml_ng::from_str(before).ok()?;
    let mut after_doc: Value = serde_yaml_ng::from_str(after).ok()?;
    let before_seq: Vec<Value> = before_doc
        .get("articles")
        .and_then(Value::as_sequence)?
        .clone();
    let before_by_number: BTreeMap<String, &Value> = before_seq
        .iter()
        .filter_map(|a| number_of(a).map(|n| (n, a)))
        .collect();
    let allowed: BTreeSet<&str> = allowed.iter().map(String::as_str).collect();
    let mut reverted = Vec::new();

    let after_seq = after_doc
        .get("articles")
        .and_then(Value::as_sequence)?
        .clone();
    let mut kept: Vec<Value> = Vec::with_capacity(after_seq.len());
    for article in after_seq {
        let number = number_of(&article);
        match number {
            Some(n) if allowed.contains(n.as_str()) => kept.push(article),
            Some(n) => match before_by_number.get(&n) {
                Some(original) if **original != article => {
                    kept.push((*original).clone());
                    reverted.push(n);
                }
                Some(_) => kept.push(article),
                // Added outside the scope: not the run's to add.
                None => reverted.push(n),
            },
            // No number: not an article anyone can review; keep it as it came.
            None => kept.push(article),
        }
    }
    // Deleted outside the scope: put back after the article that preceded it.
    let present: BTreeSet<String> = kept.iter().filter_map(number_of).collect();
    for (index, original) in before_seq.iter().enumerate() {
        let Some(n) = number_of(original) else {
            continue;
        };
        if present.contains(&n) || allowed.contains(n.as_str()) {
            continue;
        }
        let at = before_seq[..index]
            .iter()
            .rev()
            .filter_map(number_of)
            .find_map(|prev| {
                kept.iter()
                    .position(|a| number_of(a).as_deref() == Some(prev.as_str()))
            })
            .map_or(0, |p| p + 1);
        kept.insert(at, original.clone());
        reverted.push(n);
    }

    if reverted.is_empty() {
        return None;
    }
    *after_doc.get_mut("articles")? = Value::Sequence(kept);
    let text = serde_yaml_ng::to_string(&after_doc).ok()?;
    Some((text, reverted))
}

/// Bindings in the scope that dangle after the revert and did not dangle
/// before the run: a target that came to rely on something the revert took
/// away. The gates judged the document before the revert, so this is the one
/// check that sees what the reviewer is actually handed.
#[must_use]
pub fn broken_by_revert(before: &str, after: &str, allowed: &[String]) -> Vec<Signal> {
    let (Ok(before_doc), Ok(after_doc)) = (
        serde_yaml_ng::from_str::<Value>(before),
        serde_yaml_ng::from_str::<Value>(after),
    ) else {
        return Vec::new();
    };
    let had: BTreeSet<(Option<String>, String)> = super::checks::dangling_bindings(&before_doc)
        .into_iter()
        .map(|f| (f.article, f.detail))
        .collect();
    super::checks::dangling_bindings(&after_doc)
        .into_iter()
        .filter(|f| f.article.as_ref().is_some_and(|a| allowed.contains(a)))
        .filter(|f| !had.contains(&(f.article.clone(), f.detail.clone())))
        .map(|f| Signal {
            kind: "broken_by_revert".into(),
            law_id: None,
            article: f.article,
            text: format!(
                "Na het terugzetten van wijzigingen buiten de scope klopt deze binding niet \
                 meer: {}",
                f.detail
            ),
        })
        .collect()
}

/// Every output name a law body declares.
fn declared_outputs(doc: &Value) -> BTreeSet<String> {
    Graph::scan(doc).producers.into_keys().collect()
}

/// Outputs a consumer elsewhere reads that the run removed.
///
/// Only a name that existed before and is gone after counts: a consumer that
/// was already dangling is not this run's doing, and blaming it would teach
/// the reviewer to ignore the signal.
#[must_use]
pub fn consumer_breaks(ctx: &StelselContext, before: &str, after: &str) -> Vec<Signal> {
    let (Ok(before_doc), Ok(after_doc)) = (
        serde_yaml_ng::from_str::<Value>(before),
        serde_yaml_ng::from_str::<Value>(after),
    ) else {
        return Vec::new();
    };
    let had = declared_outputs(&before_doc);
    let has = declared_outputs(&after_doc);
    ctx.consumers
        .iter()
        .filter(|c| c.kind == "source_output")
        .filter(|c| had.contains(&c.target) && !has.contains(&c.target))
        .map(|c| Signal {
            kind: "consumer_breaks".into(),
            law_id: Some(c.law_id.clone()),
            article: Some(c.article.clone()),
            text: format!(
                "{} artikel {} leest output `{}` van deze wet; die output bestaat na \
                 deze verrijking niet meer.",
                c.law_id, c.article, c.target
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAW: &str = r#"
$id: test_wet
bwb_id: BWBR0000001
articles:
  - number: "1"
    text: "Definities."
  - number: "2"
    text: "Het bedrag, bedoeld in artikel 3."
    machine_readable:
      execution:
        input:
          - name: x
            source:
              output: grondslag
        output:
          - name: bedrag
  - number: "3"
    text: "De grondslag."
    machine_readable:
      execution:
        output:
          - name: grondslag
  - number: "4"
    text: "Losstaand."
  - number: "5"
    text: "Leest het bedrag."
    machine_readable:
      execution:
        input:
          - name: y
            source:
              output: bedrag
"#;

    #[test]
    fn neighbours_are_producers_readers_and_references() {
        let doc: Value = serde_yaml_ng::from_str(LAW).unwrap();
        let n = neighbours(&doc, &["2".to_string()]);
        assert_eq!(n, vec!["3".to_string(), "5".to_string()]);
    }

    #[test]
    fn revert_puts_back_edits_outside_the_scope() {
        let after = LAW.replace("Losstaand.", "Losstaand, gewijzigd.").replace(
            "          - name: bedrag\n  - number: \"3\"",
            "          - name: bedrag_nieuw\n  - number: \"3\"",
        );
        let (text, reverted) = revert_out_of_scope(LAW, &after, &["2".to_string()]).unwrap();
        assert_eq!(reverted, vec!["4".to_string()]);
        assert!(text.contains("Losstaand."));
        assert!(text.contains("bedrag_nieuw"), "the target's own edit stays");
    }

    #[test]
    fn revert_removes_added_and_restores_deleted_articles_outside_the_scope() {
        // Article 4 deleted, an article 6 added: both outside the scope of 2.
        let after = LAW
            .replace("  - number: \"4\"\n    text: \"Losstaand.\"\n", "")
            .to_string()
            + "  - number: \"6\"\n    text: \"Nieuw.\"\n";
        let (text, mut reverted) = revert_out_of_scope(LAW, &after, &["2".to_string()]).unwrap();
        reverted.sort();
        assert_eq!(reverted, vec!["4".to_string(), "6".to_string()]);
        let doc: Value = serde_yaml_ng::from_str(&text).unwrap();
        let numbers: Vec<String> = doc["articles"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(number_of)
            .collect();
        assert_eq!(
            numbers,
            vec!["1", "2", "3", "4", "5"],
            "4 back in its place, 6 gone"
        );
    }

    #[test]
    fn a_target_relying_on_a_reverted_output_is_reported() {
        // The target 2 starts reading `nieuw`, which the agent added to 4.
        let after = LAW
            .replace("              output: grondslag\n", "              output: nieuw\n")
            .replace(
                "    text: \"Losstaand.\"\n",
                "    text: \"Losstaand.\"\n    machine_readable:\n      execution:\n        output:\n          - name: nieuw\n",
            );
        let (reverted, _) = revert_out_of_scope(LAW, &after, &["2".to_string()]).unwrap();
        let signals = broken_by_revert(LAW, &reverted, &["2".to_string()]);
        assert_eq!(signals.len(), 1, "{signals:?}");
        assert_eq!(signals[0].article.as_deref(), Some("2"));
    }

    #[test]
    fn revert_is_none_when_everything_stayed_in_scope() {
        assert!(revert_out_of_scope(LAW, LAW, &["2".to_string()]).is_none());
    }

    #[test]
    fn a_removed_output_that_someone_reads_is_a_break() {
        let ctx = StelselContext {
            law_id: "test_wet".into(),
            consumers: vec![
                StelselLink {
                    law_id: "andere_wet".into(),
                    article: "7".into(),
                    target: "bedrag".into(),
                    kind: "source_output".into(),
                },
                StelselLink {
                    law_id: "andere_wet".into(),
                    article: "8".into(),
                    target: "bestond_nooit".into(),
                    kind: "source_output".into(),
                },
            ],
            ..Default::default()
        };
        let after = LAW.replace("- name: bedrag\n", "- name: hernoemd\n");
        let signals = consumer_breaks(&ctx, LAW, &after);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].article.as_deref(), Some("7"));
    }

    #[test]
    fn prompt_section_quotes_the_instruction_and_names_the_scope() {
        let scope = RunScope {
            article_mode: true,
            targets: vec!["2".into()],
            neighbours: vec!["3".into()],
            allowed: vec!["2".into(), "3".into()],
            instruction: bound_instruction(Some("  bind aan awir\nregel twee ")),
        };
        let s = scope.prompt_section();
        assert!(s.contains("entries (by their `number` field): 2."));
        assert!(s.contains("articles 3 of this law"));
        assert!(s.contains("> bind aan awir\n> regel twee"));
        assert!(s.contains("never overrides"));
    }

    #[test]
    fn instruction_is_bounded_and_empty_is_none() {
        assert_eq!(bound_instruction(Some("   ")), None);
        let long = "a".repeat(MAX_INSTRUCTION_CHARS + 10);
        assert_eq!(
            bound_instruction(Some(&long)).unwrap().chars().count(),
            MAX_INSTRUCTION_CHARS
        );
    }

    #[test]
    fn read_finds_the_stelsel_file_beside_the_law() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path()), None);
        std::fs::write(
            dir.path().join(STELSEL_FILE),
            "law_id: x\nconsumers:\n  - law_id: y\n    article: \"7\"\n    target: bedrag\n    kind: source_output\n",
        )
        .unwrap();
        let ctx = read(dir.path()).expect("read");
        assert_eq!(ctx.law_id, "x");
        assert_eq!(ctx.consumers[0].law_id, "y");
    }

    fn link(law: &str, article: &str, target: &str, kind: &str) -> StelselLink {
        StelselLink {
            law_id: law.into(),
            article: article.into(),
            target: target.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn the_section_shows_only_what_the_context_holds() {
        let empty = StelselContext {
            law_id: "x".into(),
            ..Default::default()
        };
        let bare = render_section(&empty, Some(&RunScope::default()));
        assert!(bare.contains("No article elsewhere in this corpus reads this law."));
        for absent in [
            "Neighbours you may adjust",
            "### Open terms",
            "### What this law reads elsewhere",
            "### Not included",
        ] {
            assert!(!bare.contains(absent), "{absent} in {bare}");
        }

        let full = StelselContext {
            law_id: "x".into(),
            consumers: vec![link("y", "7", "bedrag", "source_output")],
            open_terms: vec![
                OpenTermDecl {
                    id: "premie".into(),
                    article: "2".into(),
                },
                OpenTermDecl {
                    id: "drempel".into(),
                    article: "3".into(),
                },
            ],
            implementors: vec![link("regeling", "1", "premie", "implements")],
            reads: vec![link("awir", "4", "inkomen", "source_output")],
            omitted: vec!["groot".into()],
            partial: false,
        };
        let scope = RunScope {
            neighbours: vec!["3".into()],
            ..Default::default()
        };
        let text = render_section(&full, Some(&scope));
        assert!(text.contains("Neighbours you may adjust: 3."));
        assert!(text.contains("`bedrag` is read by y art. 7"));
        assert!(text.contains("`premie` (article 2), filled by regeling art. 1."));
        assert!(text.contains("`drempel` (article 3), filled by no law in this corpus."));
        assert!(text.contains("article 4 reads `inkomen` from awir"));
        assert!(text.contains("groot were left out"));

        // Implementors alone still open the section.
        let only_implementors = StelselContext {
            law_id: "x".into(),
            implementors: vec![link("regeling", "1", "premie", "implements")],
            ..Default::default()
        };
        assert!(render_section(&only_implementors, None).contains("### Open terms"));
    }

    #[test]
    fn unquoted_article_numbers_count_as_articles() {
        let before = "articles:\n  - number: 1\n    text: een\n  - number: 2\n    text: twee\n";
        let after = before.replace("text: een", "text: een\n    machine_readable: {}");
        let (_, reverted) = revert_out_of_scope(before, &after, &["2".to_string()]).unwrap();
        assert_eq!(reverted, vec!["1".to_string()]);
    }

    #[test]
    fn a_producer_or_reader_without_a_textual_reference_is_a_neighbour() {
        let law = r#"
$id: t
bwb_id: BWBR0000002
articles:
  - number: "1"
    text: "Eerste."
    machine_readable:
      execution:
        output:
          - name: a
  - number: "2"
    text: "Tweede."
    machine_readable:
      execution:
        input:
          - name: x
            source:
              output: a
        output:
          - name: b
  - number: "3"
    text: "Derde."
    machine_readable:
      execution:
        input:
          - name: y
            source:
              output: b
"#;
        let doc: Value = serde_yaml_ng::from_str(law).unwrap();
        assert_eq!(neighbours(&doc, &["2".to_string()]), vec!["1", "3"]);
        // Article 1 only produces: its neighbour is the reader, not 3.
        assert_eq!(neighbours(&doc, &["1".to_string()]), vec!["2"]);
    }

    #[test]
    fn section_says_when_absence_proves_nothing() {
        let ctx = StelselContext {
            law_id: "x".into(),
            partial: true,
            ..Default::default()
        };
        assert!(render_section(&ctx, None).contains("absence proves nothing"));
    }
}
