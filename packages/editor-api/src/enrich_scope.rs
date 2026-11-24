//! What an enrich request may ask for, and the stelselcontext that rides along.
//!
//! The editor has two ways to start an enrichment. The **article mode** comes
//! from the article panel: one article, an optional instruction from the
//! engineer, and a computed stelselcontext so the agent knows who reads the
//! law before it renames anything. The **law mode** is the classic whole-law
//! run with control over its scope: a selection of articles, or only the
//! articles that have no `machine_readable` yet.
//!
//! Everything here is pure, so the rules a request is held to are testable
//! without a database or a corpus: [`plan_request`] turns a request plus the
//! law it is about into the fields of the job payload, and
//! [`StelselBuilder`] turns the traject corpus's answers into the
//! `.enrichment-stelsel.yaml` and the slices of other laws the worker reads.
//! The worker has no corpus in the task flow, so whatever the agent should
//! see of other laws has to travel as an input blob.

use std::collections::{BTreeMap, BTreeSet};

use axum::http::StatusCode;
use serde_yaml_ng::Value;

use regelrecht_corpus::consumers_index::{BindingKind, Consumer, collect_source_bindings};
use regelrecht_pipeline::enrich::{ENRICH_MODELS, MAX_JOB_FEEDBACK_ROUNDS};
use regelrecht_pipeline::enrich_v2::stelsel::{
    MAX_INSTRUCTION_CHARS, OpenTermDecl, StelselContext, StelselLink,
};

/// Most other laws one request carries as context.
pub const MAX_CONTEXT_LAWS: usize = 16;
/// Largest slice of one other law.
pub const MAX_CONTEXT_SLICE_BYTES: usize = 256 * 1024;
/// All slices together.
pub const MAX_CONTEXT_TOTAL_BYTES: usize = 2 * 1024 * 1024;

/// Header fields a slice keeps, so the worker's lookups (by directory name in
/// the checks, by `bwb_id` + `valid_from` in the brief) still find it.
const SLICE_HEADER: &[&str] = &[
    "$schema",
    "$id",
    "name",
    "regulatory_layer",
    "publication_date",
    "valid_from",
    "bwb_id",
    "url",
];

/// Body of `POST …/laws/{law_id}/enrich`. Every field is optional, so `{}`
/// and `{provider}` keep behaving exactly as before modes existed.
#[derive(Debug, serde::Deserialize, Default)]
pub struct EnrichRequest {
    #[serde(default)]
    pub provider: Option<String>,
    /// `"article"` or `"law"` (default).
    #[serde(default)]
    pub mode: Option<String>,
    /// Article mode: exactly one top-level article. Law mode: the selection.
    #[serde(default)]
    pub articles: Option<Vec<String>>,
    /// Law mode: leave out articles that already have `machine_readable`.
    #[serde(default)]
    pub only_missing: Option<bool>,
    /// Send the stelselcontext along. Default on in the article mode, off in
    /// the law mode.
    #[serde(default)]
    pub context: Option<bool>,
    /// Feedback rounds per gate, 0 to [`MAX_JOB_FEEDBACK_ROUNDS`].
    #[serde(default)]
    pub feedback_rounds: Option<u8>,
    #[serde(default)]
    pub model: Option<String>,
    /// Article mode: what the engineer wants the agent to pay attention to.
    #[serde(default)]
    pub instruction: Option<String>,
}

/// The flag that switches the article and law mode on. Checked on the server
/// too: the frontend hides the sheet behind it, but the reason it exists is a
/// worker that ignores the scope, and any client can post the body.
pub const SCOPED_FLAG: &str = "enrich.scoped";

impl EnrichRequest {
    /// Whether the body asks for anything beyond a provider: the part that
    /// only a worker with scope support honours.
    pub fn asks_scope(&self) -> bool {
        self.mode.is_some()
            || self.articles.is_some()
            || self.only_missing.is_some()
            || self.context.is_some()
            || self.feedback_rounds.is_some()
            || self.model.is_some()
            || self.instruction.is_some()
    }
}

/// What a valid request becomes in the job payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestPlan {
    pub mode: Option<String>,
    pub articles: Option<Vec<String>>,
    pub instruction: Option<String>,
    pub feedback_rounds: Option<String>,
    pub model: Option<String>,
    pub context: bool,
}

type Refusal = (StatusCode, String);

fn bad(msg: impl Into<String>) -> Refusal {
    (StatusCode::BAD_REQUEST, msg.into())
}

/// `number:` as a string, quoted in the YAML or not.
fn number_of(article: &Value) -> Option<String> {
    match article.get("number")? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The article an entry belongs to, by the rule the worker selects entries
/// with (`entries_of` in pipeline/src/enrich.rs): the shortest existing entry
/// that `number` equals or extends with a dot. `69.1` belongs to `69` when the
/// law has a `69`, and stands on its own (as the BRP's `2.62`) when it does
/// not. Using any other cut here would let a selection name `69` and `69.1`
/// as two articles while the worker enriches `69.1` as part of `69`.
fn article_of<'a>(number: &'a str, entries: &BTreeSet<&str>) -> &'a str {
    for (i, c) in number.char_indices() {
        if c == '.' && entries.contains(&number[..i]) {
            return &number[..i];
        }
    }
    number
}

/// Top-level articles of a law in document order, each with whether any of
/// its entries carries `machine_readable`.
fn top_articles(doc: &Value) -> Vec<(String, bool)> {
    let entries: Vec<(String, bool)> = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|a| number_of(a).map(|n| (n, a.get("machine_readable").is_some())))
        .collect();
    let numbers: BTreeSet<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    let mut out: Vec<(String, bool)> = Vec::new();
    for (number, has_mr) in &entries {
        let top = article_of(number, &numbers).to_string();
        match out.iter_mut().find(|(t, _)| *t == top) {
            Some((_, mr)) => *mr |= *has_mr,
            None => out.push((top, *has_mr)),
        }
    }
    out
}

/// Check a request against the law it is about and turn it into payload
/// fields. A request without mode, selection or `only_missing` is the classic
/// unscoped walk and comes back with `articles: None`.
pub fn plan_request(
    req: &EnrichRequest,
    yaml: &str,
    provider: &str,
) -> Result<RequestPlan, Refusal> {
    let mode = match req.mode.as_deref().map(str::trim) {
        None | Some("") | Some("law") => "law",
        Some("article") => "article",
        Some(other) => {
            return Err(bad(format!(
                "Onbekende modus '{other}'; kies article of law."
            )));
        }
    };
    let model = match req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        None => None,
        Some(m) if ENRICH_MODELS.iter().any(|(p, n)| *p == provider && *n == m) => {
            Some(m.to_string())
        }
        Some(m) => {
            return Err(bad(format!(
                "Model '{m}' is niet beschikbaar voor {provider}."
            )));
        }
    };
    let feedback_rounds = match req.feedback_rounds {
        None => None,
        Some(n) if usize::from(n) <= MAX_JOB_FEEDBACK_ROUNDS => Some(n.to_string()),
        Some(n) => {
            return Err(bad(format!(
                "{n} terugkoppelrondes is te veel; het maximum is {MAX_JOB_FEEDBACK_ROUNDS}."
            )));
        }
    };
    let instruction = req
        .instruction
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty());
    if let Some(i) = instruction
        && i.chars().count() > MAX_INSTRUCTION_CHARS
    {
        return Err(bad(format!(
            "De instructie is langer dan {MAX_INSTRUCTION_CHARS} tekens."
        )));
    }

    let doc: Value = serde_yaml_ng::from_str(yaml).map_err(|_| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "De wet is geen leesbare YAML.".to_string(),
        )
    })?;
    let tops = top_articles(&doc);
    let known = |a: &str| tops.iter().any(|(t, _)| t == a);
    let requested: Option<Vec<String>> = req.articles.as_ref().map(|list| {
        let mut seen = BTreeSet::new();
        list.iter()
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty() && seen.insert(a.clone()))
            .collect()
    });
    if let Some(list) = &requested
        && let Some(missing) = list.iter().find(|a| !known(a))
    {
        return Err(bad(format!("Deze versie heeft geen artikel {missing}.")));
    }

    match mode {
        "article" => {
            let article = match requested.as_deref() {
                Some([one]) => one.clone(),
                _ => return Err(bad("De artikelmodus verrijkt precies één artikel.")),
            };
            Ok(RequestPlan {
                mode: Some("article".into()),
                articles: Some(vec![article]),
                instruction: instruction.map(str::to_string),
                feedback_rounds,
                model,
                context: req.context.unwrap_or(true),
            })
        }
        _ => {
            if instruction.is_some() {
                return Err(bad(
                    "Een instructie hoort bij de artikelmodus; geef in de wetmodus een selectie.",
                ));
            }
            let only_missing = req.only_missing.unwrap_or(false);
            let scoped = requested.is_some() || only_missing;
            let articles = if scoped {
                // Document order, whatever order the request listed them in.
                let wanted: Vec<String> = tops
                    .iter()
                    .filter(|(t, _)| requested.as_ref().is_none_or(|r| r.contains(t)))
                    .filter(|(_, has_mr)| !(only_missing && *has_mr))
                    .map(|(t, _)| t.clone())
                    .collect();
                if wanted.is_empty() {
                    return Err((
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "Niets te verrijken: elk gekozen artikel heeft al machine_readable."
                            .to_string(),
                    ));
                }
                Some(wanted)
            } else {
                None
            };
            Ok(RequestPlan {
                mode: scoped.then(|| "law".to_string()),
                articles,
                instruction: None,
                feedback_rounds,
                model,
                context: req.context.unwrap_or(false),
            })
        }
    }
}

/// Open terms a law declares, by article.
pub fn open_terms_of(doc: &Value) -> Vec<OpenTermDecl> {
    let mut out = Vec::new();
    for article in doc
        .get("articles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        let Some(number) = number_of(article) else {
            continue;
        };
        for term in article
            .get("machine_readable")
            .and_then(|mr| mr.get("open_terms"))
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            if let Some(id) = term.get("id").and_then(Value::as_str) {
                out.push(OpenTermDecl {
                    id: id.to_string(),
                    article: number.clone(),
                });
            }
        }
    }
    out
}

fn kind_label(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::SourceOutput => "source_output",
        BindingKind::SourceArticle => "source_article",
        BindingKind::Implements => "implements",
    }
}

/// A slice of another law: its header, and only the articles named or those
/// declaring one of the named outputs. `None` when nothing of it is relevant.
pub fn slice_law(yaml: &str, wanted: &Wanted) -> Option<String> {
    let doc: Value = serde_yaml_ng::from_str(yaml).ok()?;
    let mut map = serde_yaml_ng::Mapping::new();
    for key in SLICE_HEADER {
        if let Some(v) = doc.get(*key) {
            map.insert(Value::String((*key).to_string()), v.clone());
        }
    }
    let names_in = |article: &Value, path: [&str; 2], field: &str, set: &BTreeSet<String>| {
        article
            .get("machine_readable")
            .and_then(|mr| {
                if path[0].is_empty() {
                    Some(mr)
                } else {
                    mr.get(path[0])
                }
            })
            .and_then(|v| v.get(path[1]))
            .and_then(Value::as_sequence)
            .is_some_and(|seq| {
                seq.iter().any(|o| {
                    o.get(field)
                        .and_then(Value::as_str)
                        .is_some_and(|n| set.contains(n))
                })
            })
    };
    let kept: Vec<Value> = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter(|a| {
            number_of(a).is_some_and(|n| {
                wanted.articles.contains(&n)
                    || wanted
                        .articles
                        .iter()
                        .any(|w| n.starts_with(&format!("{w}.")))
            }) || names_in(a, ["execution", "output"], "name", &wanted.outputs)
                || names_in(a, ["", "open_terms"], "id", &wanted.open_terms)
        })
        .cloned()
        .collect();
    if kept.is_empty() {
        return None;
    }
    map.insert(
        Value::String(regelrecht_pipeline::enrich_v2::stelsel::SLICE_MARKER.into()),
        Value::Bool(true),
    );
    map.insert(Value::String("articles".into()), Value::Sequence(kept));
    serde_yaml_ng::to_string(&Value::Mapping(map)).ok()
}

/// What a slice of one other law should keep, and how near that law is.
#[derive(Debug, Default)]
pub struct Wanted {
    /// 0 for a law that reads this one (consumer, implementor), 1 for a law
    /// this one reads. The nearer laws go first when the budget runs out:
    /// they are the ones a change here can break.
    priority: u8,
    /// Article numbers to keep (and their entries).
    articles: BTreeSet<String>,
    /// Articles declaring one of these outputs are kept.
    outputs: BTreeSet<String>,
    /// Articles declaring one of these open terms are kept.
    open_terms: BTreeSet<String>,
}

/// What to fetch from each other law, collected from consumers, implementors
/// and the law's own reads.
#[derive(Debug, Default)]
pub struct StelselBuilder {
    pub context: StelselContext,
    wanted: BTreeMap<String, Wanted>,
}

impl StelselBuilder {
    /// Start from the law being enriched and the consumers the traject index
    /// found for it.
    pub fn new(law_id: &str, yaml: &str, consumers: Vec<Consumer>, partial: bool) -> Self {
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap_or(Value::Null);
        let mut builder = Self {
            context: StelselContext {
                law_id: law_id.to_string(),
                open_terms: open_terms_of(&doc),
                partial,
                ..Default::default()
            },
            wanted: BTreeMap::new(),
        };
        for c in consumers {
            let link = StelselLink {
                law_id: c.law_id.clone(),
                article: c.article.clone(),
                target: c.target.clone(),
                kind: kind_label(c.kind).to_string(),
            };
            builder.want(&c.law_id, 0).articles.insert(c.article);
            match c.kind {
                BindingKind::Implements => builder.context.implementors.push(link),
                _ => builder.context.consumers.push(link),
            }
        }
        for b in collect_source_bindings(yaml) {
            let entry = builder.want(&b.regulation, 1);
            match b.kind {
                BindingKind::SourceOutput => {
                    entry.outputs.insert(b.target.clone());
                }
                BindingKind::SourceArticle => {
                    entry.articles.insert(b.target.clone());
                }
                // This law fills an open term of the higher law: keep the
                // article there that declares it.
                BindingKind::Implements => {
                    entry.open_terms.insert(b.target.clone());
                }
            }
            builder.context.reads.push(StelselLink {
                law_id: b.regulation,
                article: b.article,
                target: b.target,
                kind: kind_label(b.kind).to_string(),
            });
        }
        builder
    }

    /// The entry for `law_id`, keeping the nearest priority seen.
    fn want(&mut self, law_id: &str, priority: u8) -> &mut Wanted {
        let entry = self
            .wanted
            .entry(law_id.to_string())
            .or_insert_with(|| Wanted {
                priority,
                ..Default::default()
            });
        entry.priority = entry.priority.min(priority);
        entry
    }

    /// The other laws whose slices are wanted, nearest first: consumers and
    /// implementors before what the law reads, then by id. The caller fetches
    /// at most [`MAX_CONTEXT_LAWS`]; [`Self::finish`] names the rest.
    pub fn laws(&self) -> Vec<String> {
        let mut laws: Vec<(&u8, &String)> = self
            .wanted
            .iter()
            .map(|(id, w)| (&w.priority, id))
            .collect();
        laws.sort();
        laws.into_iter().map(|(_, id)| id.clone()).collect()
    }

    /// Slice the fetched bodies within the budgets, nearest laws first.
    /// `bodies` maps a law to its YAML, or to `None` when it could not be
    /// read; a law missing from `bodies` was not fetched. Returns the context
    /// file and the `(law_id, slice)` pairs to store.
    pub fn finish(
        mut self,
        bodies: &BTreeMap<String, Option<String>>,
    ) -> (StelselContext, Vec<(String, String)>) {
        let mut slices = Vec::new();
        let mut total = 0usize;
        for law_id in self.laws() {
            let slice = bodies
                .get(&law_id)
                .and_then(Option::as_deref)
                .and_then(|yaml| slice_law(yaml, &self.wanted[&law_id]));
            match slice {
                Some(slice)
                    if slices.len() < MAX_CONTEXT_LAWS
                        && slice.len() <= MAX_CONTEXT_SLICE_BYTES
                        && total + slice.len() <= MAX_CONTEXT_TOTAL_BYTES =>
                {
                    total += slice.len();
                    slices.push((law_id, slice));
                }
                _ => self.context.omitted.push(law_id),
            }
        }
        self.context.omitted.sort();
        (self.context, slices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAW: &str = r#"
$id: test_wet
bwb_id: BWBR0000001
valid_from: '2025-01-01'
articles:
  - number: "1"
    text: Definities.
  - number: "2"
    text: Twee.
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
      execution:
        input:
          - name: inkomen
            source:
              regulation: awir
              output: toetsingsinkomen
  - number: "2.a"
    text: Twee a.
  - number: "3"
    text: Drie.
"#;

    fn req(json: serde_json::Value) -> EnrichRequest {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn entries_group_under_the_article_the_worker_would_enrich() {
        let doc: Value = serde_yaml_ng::from_str(
            "articles:\n  - number: \"69\"\n  - number: \"69.1\"\n  - number: \"2.62\"\n  - number: \"2.63\"\n",
        )
        .unwrap();
        let tops: Vec<String> = top_articles(&doc).into_iter().map(|(t, _)| t).collect();
        assert_eq!(tops, vec!["69", "2.62", "2.63"]);
    }

    #[test]
    fn the_slice_of_a_higher_law_keeps_the_article_with_the_open_term() {
        let regeling = r#"
$id: regeling
articles:
  - number: "1"
    machine_readable:
      implements:
        - law: wet_hoger
          open_term: standaardpremie
"#;
        let builder = StelselBuilder::new("regeling", regeling, Vec::new(), false);
        let hoger = r#"
$id: wet_hoger
articles:
  - number: "4"
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
  - number: "5"
    text: Niet nodig.
"#;
        let bodies = BTreeMap::from([("wet_hoger".to_string(), Some(hoger.to_string()))]);
        let (ctx, slices) = builder.finish(&bodies);
        assert!(ctx.omitted.is_empty());
        assert!(slices[0].1.contains("standaardpremie"));
        assert!(!slices[0].1.contains("Niet nodig"));
        assert!(
            slices[0]
                .1
                .contains(regelrecht_pipeline::enrich_v2::stelsel::SLICE_MARKER)
        );
    }

    #[test]
    fn an_empty_body_is_the_classic_walk() {
        let plan = plan_request(&EnrichRequest::default(), LAW, "claude").unwrap();
        assert_eq!(plan, RequestPlan::default());
    }

    #[test]
    fn the_article_mode_takes_one_existing_article_and_its_instruction() {
        let plan = plan_request(
            &req(serde_json::json!({"mode": "article", "articles": ["2"], "instruction": " let op lid 2 "})),
            LAW,
            "claude",
        )
        .unwrap();
        assert_eq!(plan.articles, Some(vec!["2".to_string()]));
        assert_eq!(plan.instruction.as_deref(), Some("let op lid 2"));
        assert!(plan.context, "context defaults on in the article mode");

        for bad_req in [
            serde_json::json!({"mode": "article"}),
            serde_json::json!({"mode": "article", "articles": ["2", "3"]}),
            serde_json::json!({"mode": "article", "articles": ["9"]}),
            serde_json::json!({"mode": "elders"}),
        ] {
            let err = plan_request(&req(bad_req.clone()), LAW, "claude").unwrap_err();
            assert_eq!(err.0, StatusCode::BAD_REQUEST, "{bad_req}");
        }
    }

    #[test]
    fn only_missing_drops_enriched_articles_and_keeps_document_order() {
        let plan = plan_request(
            &req(serde_json::json!({"articles": ["3", "2", "1"], "only_missing": true})),
            LAW,
            "claude",
        )
        .unwrap();
        assert_eq!(plan.mode.as_deref(), Some("law"));
        assert_eq!(plan.articles, Some(vec!["1".to_string(), "3".to_string()]));

        let err = plan_request(
            &req(serde_json::json!({"articles": ["2"], "only_missing": true})),
            LAW,
            "claude",
        )
        .unwrap_err();
        assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn model_rounds_and_instruction_are_bounded() {
        let ok = plan_request(
            &req(serde_json::json!({"model": "claude-sonnet-5-5", "feedback_rounds": 2})),
            LAW,
            "claude",
        )
        .unwrap();
        assert_eq!(ok.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(ok.feedback_rounds.as_deref(), Some("2"));
        for bad_req in [
            serde_json::json!({"model": "claude-sonnet-5-5", "provider": "opencode"}),
            serde_json::json!({"feedback_rounds": 9}),
            serde_json::json!({"instruction": "alleen in artikelmodus"}),
            serde_json::json!({"mode": "article", "articles": ["2"], "instruction": "x".repeat(MAX_INSTRUCTION_CHARS + 1)}),
        ] {
            let provider = bad_req
                .get("provider")
                .and_then(|p| p.as_str())
                .unwrap_or("claude")
                .to_string();
            assert!(
                plan_request(&req(bad_req.clone()), LAW, &provider).is_err(),
                "{bad_req}"
            );
        }
    }

    #[test]
    fn the_builder_collects_consumers_implementors_reads_and_open_terms() {
        let consumers = vec![
            Consumer {
                law_id: "andere_wet".into(),
                article: "7".into(),
                target: "bedrag".into(),
                kind: BindingKind::SourceOutput,
            },
            Consumer {
                law_id: "regeling".into(),
                article: "1".into(),
                target: "standaardpremie".into(),
                kind: BindingKind::Implements,
            },
        ];
        let builder = StelselBuilder::new("test_wet", LAW, consumers, false);
        // Readers first (andere_wet, regeling), then what the law reads (awir).
        assert_eq!(builder.laws(), vec!["andere_wet", "regeling", "awir"]);
        let awir = r#"
$id: awir
bwb_id: BWBR0018450
articles:
  - number: "8"
    machine_readable:
      execution:
        output:
          - name: toetsingsinkomen
  - number: "9"
    text: Niet nodig.
"#;
        let bodies = BTreeMap::from([
            ("awir".to_string(), Some(awir.to_string())),
            ("andere_wet".to_string(), None),
        ]);
        let (ctx, slices) = builder.finish(&bodies);
        assert_eq!(ctx.open_terms[0].id, "standaardpremie");
        assert_eq!(ctx.consumers.len(), 1);
        assert_eq!(ctx.implementors.len(), 1);
        assert_eq!(ctx.reads[0].target, "toetsingsinkomen");
        assert_eq!(ctx.omitted, vec!["andere_wet", "regeling"]);
        assert_eq!(slices.len(), 1);
        assert!(slices[0].1.contains("toetsingsinkomen"));
        assert!(
            !slices[0].1.contains("Niet nodig"),
            "only the producing article"
        );
        assert!(
            slices[0].1.contains("bwb_id"),
            "the header survives the slice"
        );
    }
}
