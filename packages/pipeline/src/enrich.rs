use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use regelrecht_law_model::ArticleBasedLaw;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::enrich_v2::{capabilities, context};
use crate::error::{PipelineError, Result};

/// Per-process cache of branch names already confirmed to exist on the
/// corpus remote. Branches are never deleted once created, so a positive
/// probe is permanent for the life of the worker — caching skips the
/// ls-remote round-trip on every subsequent enrich job for the same PR.
fn known_remote_branches() -> &'static Mutex<HashSet<String>> {
    static CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn branch_is_known(branch: &str) -> bool {
    known_remote_branches()
        .lock()
        .map(|cache| cache.contains(branch))
        .unwrap_or(false)
}

fn remember_branch(branch: &str) {
    if let Ok(mut cache) = known_remote_branches().lock() {
        cache.insert(branch.to_string());
    }
}

/// Pick the base branch to check out the law YAML from, given the worker's
/// preferred branch and whether that branch exists on the remote. Pure
/// function so the branch-selection contract can be pinned by unit tests
/// without a live git remote.
fn pick_enrich_base(preferred: &str, preferred_exists: bool) -> &str {
    if preferred == "development" || preferred_exists {
        preferred
    } else {
        "development"
    }
}

/// Outcome of comparing a target law's base version against the enrichment's
/// recorded provenance. Pure decision so it can be unit-tested without git.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseAction {
    /// Law not yet on the enrich branch — check it out fresh from the base.
    CheckoutFresh,
    /// Law present and its recorded base matches the current base — unchanged,
    /// keep the existing enrichment (no fresh checkout).
    Skip,
    /// Law present but with no usable recorded provenance — a "legacy"
    /// enrichment written before the freshness guard existed. Adopt the current
    /// base blob SHA as its baseline (recorded on the next metadata write) and
    /// proceed without a fresh checkout, rather than failing. This grandfathers
    /// every pre-guard enrichment so introducing the guard does not turn them
    /// all into an immediate `Drift` on first re-enrichment.
    AdoptBaseline,
    /// Law present and its *recorded* base moved — fail loudly rather than
    /// silently re-enriching on top of a base that differs from the one the
    /// enrichment was generated against.
    Drift,
}

/// Decide what to do for a target law given whether it is already tracked on
/// the enrich branch, the `source_hash` recorded in its `.enrichment.yaml`
/// (if any), and the current base-branch blob SHA of the law.
pub(crate) fn decide_base_action(
    tracked: bool,
    stored_source_hash: Option<&str>,
    base_sha: &str,
) -> BaseAction {
    if !tracked {
        return BaseAction::CheckoutFresh;
    }
    match stored_source_hash {
        // No usable provenance recorded (absent or empty) — a pre-guard
        // enrichment. Grandfather it by adopting the current base as its
        // baseline instead of treating the unknown as drift.
        None | Some("") => BaseAction::AdoptBaseline,
        // Recorded base matches the current base — unchanged.
        Some(h) if h == base_sha => BaseAction::Skip,
        // Recorded base differs from the current base — genuine drift.
        Some(_) => BaseAction::Drift,
    }
}

/// The article window one enrich run must process, decided by the worker (not
/// the LLM) from the persisted cursor. Pure decision so the chunking contract
/// can be pinned by unit tests without git or an LLM (pattern:
/// [`decide_base_action`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkPlan {
    /// Chunking disabled (`ENRICH_MAX_ARTICLES_PER_RUN=0`): process the whole
    /// law in one session, exactly the pre-chunking behavior.
    WholeLaw,
    /// Process articles `[start, end)` in document order. `law_complete` is
    /// true when this window reaches the end of the document — the law can be
    /// marked `enriched` after this run.
    Chunk {
        start: usize,
        end: usize,
        law_complete: bool,
    },
}

/// Plan the article window for this run.
///
/// The stored cursor only counts when it was recorded for the *same* law YAML
/// path AND still fits the document (`cursor <= articles_total`); anything else
/// (new law version at another path, corrupt metadata, legacy files without
/// cursor fields) resets to 0. The window is document order from the cursor —
/// deliberately NOT "the first N un-enriched articles": the law-generate skill
/// legitimately skips definition/procedure/transitional articles without
/// `machine_readable`, so an un-enriched-first window would revisit the same
/// skipped articles forever and never terminate. A cursor guarantees
/// termination in `ceil(total / N)` successful runs regardless of LLM behavior.
///
/// # Samenhang van één hoofdartikel
///
/// `entries` geeft de nummers in documentvolgorde. Staat het mee, dan wordt de
/// rechterrand van het venster vooruit geschoven zolang de volgende entry tot
/// hetzelfde hoofdartikel behoort als de laatste in het venster. Een aanhef
/// komt daarmee altijd in één venster met zijn eigen leden en onderdelen.
///
/// Dat is geen esthetiek maar de grootste enkele maatregel tegen bindingen die
/// een venster niet kan leggen. Op het corpus van ronde 4 zit **48 van de 57**
/// vooruitwijzende intra-wet bindingen binnen één hoofdartikel; bij de
/// venstermaat die vandaag draait daalt het aantal bindingen dat buiten zijn
/// venster valt van 46 naar 8. Een herordening van de artikelen doet dat niet:
/// gemeten bracht een topologische orde uit de verwijzingsgraaf die 57 juist op
/// 69 (zie [`crate::enrich_v2::refgraph`]).
///
/// De terminatiegarantie blijft ongemoeid. Het opschuiven maakt een venster
/// alleen groter, nooit kleiner, dus elke geslaagde run verzet de cursor met
/// ten minste `max_articles_per_run` entries of bereikt het einde: nog steeds
/// ten hoogste `ceil(total / N)` runs. Een lege `entries` betekent dat de
/// aanroeper de nummers niet meegaf en levert precies het gedrag van vóór deze
/// regel.
pub(crate) fn plan_chunk(
    max_articles_per_run: usize,
    articles_total: usize,
    stored_cursor: usize,
    stored_cursor_path: &str,
    yaml_path: &str,
    entries: &[String],
) -> ChunkPlan {
    if max_articles_per_run == 0 {
        return ChunkPlan::WholeLaw;
    }
    let start = if stored_cursor_path == yaml_path && stored_cursor <= articles_total {
        stored_cursor
    } else {
        0
    };
    let mut end = start
        .saturating_add(max_articles_per_run)
        .min(articles_total);
    if entries.len() == articles_total {
        while end > start && end < articles_total {
            let last = crate::enrich_v2::refgraph::entry_group(&entries[end - 1]);
            let next = crate::enrich_v2::refgraph::entry_group(&entries[end]);
            if last != next {
                break;
            }
            end += 1;
        }
    }
    ChunkPlan::Chunk {
        start,
        end,
        law_complete: end >= articles_total,
    }
}

/// What a window is.
///
/// Today a window is "the next N entries in document order", and that is the
/// reason an entry is visited before the value it has to bind to exists: the
/// cut runs straight through the dependencies. The alternative is to derive
/// the window instead of guessing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMode {
    /// The next `ENRICH_MAX_ARTICLES_PER_RUN` entries from the cursor, with the
    /// boundary snapped so a top-level article is never cut in half.
    #[default]
    Document,
    /// One layer of the reference graph: a set of top-level articles none of
    /// which depends on another in the same set, with the layers walked in
    /// dependency order. Its size follows from the law rather than from a
    /// constant, and a cycle is one honestly derived layer instead of an
    /// arbitrary cut.
    ///
    /// Off by default, and the measurement is the reason. On the round-4
    /// corpus a layer order raised the number of intra-law bindings pointing
    /// forward from 57 to 69: document order is already a good build order,
    /// because the legislator puts definitions first while a large share of
    /// the reference edges ("in afwijking van artikel 8") run the other way
    /// and carry no value at all. The mode is here so that claim can be
    /// re-measured on other laws, not because it is the better default.
    Layer,
}

impl WindowMode {
    /// Parse the `ENRICH_WINDOW_MODE` spec.
    pub fn parse(spec: &str) -> std::result::Result<Self, String> {
        match spec.trim().to_lowercase().as_str() {
            "document" | "" => Ok(Self::Document),
            "layer" => Ok(Self::Layer),
            other => Err(format!("unknown window mode: {other}")),
        }
    }

    /// Stable name, used in logs and recorded beside the cursor.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Layer => "layer",
        }
    }
}

/// The entries of layer `index`, and whether it is the last one.
///
/// The cursor counts layers in this mode instead of entries, which keeps the
/// termination property in the same shape: every successful run raises it by
/// one and there is a fixed number of layers, so the walk ends in at most
/// `layers()` runs. A layer is not contiguous in document order, which is why
/// the window travels as a list of entry numbers and not as a range — the
/// payload already carried it that way.
pub(crate) fn plan_layer_window(
    graph: &crate::enrich_v2::refgraph::Graph,
    entries: &[String],
    index: usize,
) -> (Vec<String>, bool) {
    use crate::enrich_v2::refgraph::entry_group;

    let layers = graph.layers();
    let Some(layer) = layers.get(index) else {
        return (Vec::new(), true);
    };
    let numbers = entries
        .iter()
        .filter(|entry| layer.iter().any(|a| a == entry_group(entry)))
        .cloned()
        .collect();
    (numbers, index + 1 >= layers.len())
}

/// Split one window over at most `concurrency` agents.
///
/// The split is by top-level article, never inside one: an aanhef and its
/// leden belong to the same agent for the same reason they belong to the same
/// window. And it only happens when no two articles in the window reference
/// each other — two agents that both have to name the same concept will name
/// it differently, and two independently invented names for one concept never
/// find each other again. That is a silent hole no later pass detects, which
/// makes it worse than the ordering problem it would be trading against.
///
/// `concurrency` of 1, the default, always returns the window whole: one agent
/// pays the fixed per-session cost once.
pub(crate) fn split_window(
    graph: &crate::enrich_v2::refgraph::Graph,
    numbers: &[String],
    concurrency: usize,
) -> Vec<Vec<String>> {
    use crate::enrich_v2::refgraph::entry_group;

    if concurrency <= 1 || numbers.len() < 2 {
        return vec![numbers.to_vec()];
    }
    let mut articles: Vec<&str> = Vec::new();
    for number in numbers {
        let top = entry_group(number);
        if !articles.contains(&top) {
            articles.push(top);
        }
    }
    if articles.len() < 2 {
        return vec![numbers.to_vec()];
    }
    // One related pair is enough to keep the whole window together: splitting
    // around it would be a judgement about which side owns the name.
    for (i, a) in articles.iter().enumerate() {
        for b in articles.iter().skip(i + 1) {
            if graph.related(a, b) {
                return vec![numbers.to_vec()];
            }
        }
    }
    let buckets = concurrency.min(articles.len());
    let mut out: Vec<Vec<String>> = vec![Vec::new(); buckets];
    for (index, article) in articles.iter().enumerate() {
        let bucket = index % buckets;
        out[bucket].extend(
            numbers
                .iter()
                .filter(|n| entry_group(n) == *article)
                .cloned(),
        );
    }
    out.retain(|w| !w.is_empty());
    out
}

/// Fold the per-window copies of a law back into one file.
///
/// Each window worked on its own copy, so the merge is entry-wise: for every
/// window, the entries it was assigned are taken from its copy and everything
/// else from the base. Disjoint by construction, so there is nothing to
/// resolve — and that is exactly what has to be proved rather than assumed.
///
/// The merge **refuses** rather than guesses. A window that changed an entry
/// outside its own assignment is an error with that entry's number in it, and
/// so is a copy that added or dropped entries. Round 3 lost four runs to two
/// agents writing the same file; a merge that silently picked a winner would
/// reproduce that with better manners.
pub(crate) fn merge_windows(
    base: &str,
    windows: &[(Vec<String>, String)],
) -> std::result::Result<String, String> {
    use serde_yaml_ng::Value;

    let mut merged: Value =
        serde_yaml_ng::from_str(base).map_err(|e| format!("base law is not YAML: {e}"))?;
    let base_articles = articles_by_number(&merged)?;

    let mut taken: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut replacements: std::collections::BTreeMap<String, Value> =
        std::collections::BTreeMap::new();
    for (index, (assigned, text)) in windows.iter().enumerate() {
        let doc: Value = serde_yaml_ng::from_str(text)
            .map_err(|e| format!("window {index} left a file that is not YAML: {e}"))?;
        let theirs = articles_by_number(&doc)?;
        if theirs.len() != base_articles.len() {
            return Err(format!(
                "window {index} changed the number of entries (was {}, is {})",
                base_articles.len(),
                theirs.len()
            ));
        }
        for (number, article) in &theirs {
            let Some(before) = base_articles.get(number) else {
                return Err(format!("window {index} introduced entry {number}"));
            };
            if article == before {
                continue;
            }
            if !assigned.iter().any(|a| a == number) {
                return Err(format!(
                    "window {index} changed entry {number}, which was not assigned to it"
                ));
            }
            if let Some(other) = taken.insert(number.clone(), index) {
                return Err(format!(
                    "windows {other} and {index} both changed entry {number}"
                ));
            }
            replacements.insert(number.clone(), article.clone());
        }
    }

    if let Some(Value::Sequence(articles)) = merged.get_mut("articles") {
        for article in articles.iter_mut() {
            let Some(number) = article.get("number").and_then(Value::as_str) else {
                continue;
            };
            if let Some(new) = replacements.get(number) {
                *article = new.clone();
            }
        }
    }
    serde_yaml_ng::to_string(&merged).map_err(|e| format!("cannot serialise the merge: {e}"))
}

/// Entries keyed by number, for the merge.
fn articles_by_number(
    doc: &serde_yaml_ng::Value,
) -> std::result::Result<std::collections::BTreeMap<String, serde_yaml_ng::Value>, String> {
    let mut out = std::collections::BTreeMap::new();
    for article in doc
        .get("articles")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .into_iter()
        .flatten()
    {
        let number = article
            .get("number")
            .and_then(serde_yaml_ng::Value::as_str)
            .ok_or_else(|| "an entry has no number".to_string())?;
        out.insert(number.to_string(), article.clone());
    }
    Ok(out)
}

/// Trait abstracting the LLM invocation so `execute_enrich` can be tested
/// with a fake provider that doesn't spawn real processes.
#[async_trait::async_trait]
pub trait LlmRunner: Send + Sync {
    /// Run the LLM on the given YAML file and return its exit status.
    ///
    /// Implementations should respect the timeout in `config`.
    async fn run(
        &self,
        payload: &EnrichPayload,
        yaml_abs: &Path,
        repo_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()>;
}

/// What one agent run cost, as the provider reports it.
///
/// Round 3 could compare variants on outcome and wall clock and on nothing
/// else, so the question whether the context brief is expensive because the
/// input grew or because the agent did more work stayed open. The provider
/// already answers that on its own stdout, which this worker drained and threw
/// away. RFC-028 asks for it under "every agent run must be countable".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Tokens served from the provider's prompt cache. Large here, because
    /// every window resends the same skill files.
    pub cache_read_tokens: u64,
    /// Tokens written into the prompt cache. Counted separately because it is
    /// billed above the plain input rate where a cache read is billed well
    /// below it, so the two must never be added together.
    pub cache_write_tokens: u64,
    /// Cost in thousandths of a cent, so the figure stays an integer. The provider
    /// reports dollars as a float and money in a float is a bug waiting.
    pub cost_millicents: u64,
}

impl AgentUsage {
    /// Read the usage out of the provider's final JSON object.
    ///
    /// Takes the tail of stdout rather than the whole stream: the object we
    /// want is the last one, and opencode inlines multi-megabyte bodies
    /// earlier in the stream that nobody should hold in memory to reach it.
    /// Returns `None` when the tail carries no recognisable object, which is
    /// the normal case for a provider that reports nothing.
    #[must_use]
    pub fn from_stdout_tail(tail: &str) -> Option<Self> {
        // The whole tail first. `--output-format json` writes one object, and
        // looking for the last `{` lands inside it: this payload nests
        // `iterations` and `cache_creation`, so the scan found a fragment with
        // no `usage` and every figure came out zero.
        //
        // The scan stays for the streaming shape, where the closing object
        // really is the last one on the stream.
        //
        // Then line by line from the end, for stream-json: the last line
        // with a top-level `usage` is the closing `result`. Assistant and
        // stream events carry theirs nested, and the CLI puts `type`
        // anywhere in the object, so neither key order nor a `{"type"`
        // prefix can be relied on.
        let value: serde_json::Value = serde_json::from_str(tail.trim())
            .ok()
            .or_else(|| {
                tail.lines().rev().find_map(|line| {
                    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
                    v.get("usage").is_some().then_some(v)
                })
            })
            .or_else(|| {
                let start = tail.rfind("{\"type\"").or_else(|| tail.rfind('{'))?;
                serde_json::from_str(tail[start..].trim()).ok()
            })?;
        let usage = value.get("usage")?;
        let n = |key: &str| {
            usage
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        Some(Self {
            input_tokens: n("input_tokens"),
            output_tokens: n("output_tokens"),
            cache_read_tokens: n("cache_read_input_tokens"),
            cache_write_tokens: n("cache_creation_input_tokens"),
            cost_millicents: value
                .get("total_cost_usd")
                .and_then(serde_json::Value::as_f64)
                .map_or(0, |usd| (usd * 100_000.0).round() as u64),
        })
    }

    /// Add a second run's usage, for reporting a whole chain as one figure.
    #[must_use]
    pub fn plus(self, other: Self) -> Self {
        Self {
            input_tokens: self.input_tokens + other.input_tokens,
            output_tokens: self.output_tokens + other.output_tokens,
            cache_read_tokens: self.cache_read_tokens + other.cache_read_tokens,
            cache_write_tokens: self.cache_write_tokens + other.cache_write_tokens,
            cost_millicents: self.cost_millicents + other.cost_millicents,
        }
    }
}

/// Whether the calls in one window share one agent session.
///
/// A window is one law and one article range. The translation pass and the
/// feedback rounds that follow it argue about the same articles, with the same
/// skills and the same context brief, and every one of them is a cold process
/// that reads all of it again: up to seven starts per window, five windows for
/// the zorgtoeslag. Continuing the session instead is what this setting buys.
///
/// Never across windows. An agent that keeps everything it wrote carries half
/// the Awir into the last window, and that is dearer than starting over.
///
/// Whether reuse is cheaper is a question with a number behind it, not a
/// given. In the round-3/4 transcripts a cold feedback round took a median 43
/// turns over a context that started near nothing, and a translation pass
/// ended at ~134k tokens. A resumed round pays every one of its turns over
/// that ending context, so it only comes out ahead if knowing the law already
/// cuts it to about 17 turns or fewer. That is what the per-call accounting on
/// [`EnrichResult`] exists to settle, and why `off` stays one env var away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionReuse {
    /// Every call is its own cold process — the behaviour before this existed,
    /// and the way back if reuse turns out to cost more than it saves.
    Off,
    /// The translation pass and the two hard gates (schema and binding) share
    /// a session; the checks and marking gates stay cold. A schema error or a
    /// reference to nothing is a fact about the file and repairing it asks for
    /// no fresh judgement, while those two gates ask
    /// the agent to look again at a choice it made — which is the one thing an
    /// agent that remembers making it is worst at.
    Repair,
    /// Every call in the window continues the same session. The default: the
    /// budget is going on the rounds, and withholding reuse from the rounds
    /// keeps almost none of it. What guards the fresh look here is the
    /// `REREAD_INSTRUCTION` each resumed feedback prompt opens with.
    #[default]
    Window,
}

impl SessionReuse {
    /// Parse `ENRICH_SESSION_REUSE`.
    pub fn parse(spec: &str) -> std::result::Result<Self, String> {
        match spec.trim() {
            "off" | "0" => Ok(Self::Off),
            "repair" => Ok(Self::Repair),
            "window" | "1" => Ok(Self::Window),
            other => Err(format!("unknown session reuse mode: {other}")),
        }
    }

    /// Stable lowercase name for logs and the measurement record.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Repair => "repair",
            Self::Window => "window",
        }
    }
}

/// What a resumed feedback prompt opens with.
///
/// The point of a gate is a fresh look at what stands in the file. An agent
/// that still remembers why it wrote something can answer from that memory and
/// defend the choice instead of reading the finding — the exact failure the
/// gates exist to catch. It cannot be ruled out from here, so it is met head
/// on: the first thing the resumed prompt says is that memory is not evidence
/// and the file is.
const REREAD_INSTRUCTION: &str = "Read the article you are about to change from the file before \
     you answer, even though you wrote it yourself earlier in this conversation. What you remember \
     writing is not evidence of what is in the file: the finding below comes from a check that ran \
     over the file as it now stands, and other rounds may have touched it since. Judge the finding \
     against what you read there, not against what you meant to write. If the finding is right, \
     change the file; do not explain why the earlier choice was defensible.";

/// What one call must do with the window's session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionAction {
    /// No session id at all: the provider opens and forgets its own.
    Cold,
    /// Open the window's session under an id the worker chose.
    Start(Uuid),
    /// Continue the window's session.
    Resume(Uuid),
}

impl SessionAction {
    /// Whether this call continues an existing conversation.
    fn resumed(self) -> bool {
        matches!(self, Self::Resume(_))
    }
}

/// What one call to the agent was and what it cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCallRecord {
    /// `translate`, or the label of the gate this feedback round answered.
    pub step: String,
    /// Whether the call continued the window's session rather than starting
    /// cold. Reading a run means comparing these two against each other.
    pub resumed: bool,
    /// What the provider reported, or `None` when it reported nothing.
    pub usage: Option<AgentUsage>,
    /// The model the call ran on, as the CLI named it in its `init` event.
    /// `None` when it named none (opencode, a fake runner).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// The agent session of one window: the id every call in it shares, whether it
/// has been opened yet, and what each call cost.
///
/// Owned by `execute_enrich_with_runner` and handed to the runner on the
/// payload, so it lives exactly as long as the window does.
#[derive(Debug)]
pub struct AgentSession {
    id: Uuid,
    reuse: SessionReuse,
    state: Mutex<SessionState>,
}

#[derive(Debug, Default)]
struct SessionState {
    /// True once a call has successfully opened the session, so the next
    /// shareable call resumes it instead of starting a second one.
    open: bool,
    calls: Vec<AgentCallRecord>,
}

impl AgentSession {
    #[must_use]
    pub fn new(reuse: SessionReuse) -> Self {
        Self {
            id: Uuid::new_v4(),
            reuse,
            state: Mutex::new(SessionState::default()),
        }
    }

    /// The id every call in this window shares.
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    #[must_use]
    pub fn reuse(&self) -> SessionReuse {
        self.reuse
    }

    /// Decide what this call does with the session. Pure with respect to the
    /// session's own state, which only changes when the call is recorded.
    pub(crate) fn plan(&self, pass: &Pass) -> SessionAction {
        let shareable = match (self.reuse, pass) {
            (SessionReuse::Off, _) => false,
            (_, Pass::Translate) => true,
            (SessionReuse::Window, Pass::Feedback(_)) => true,
            (SessionReuse::Repair, Pass::Feedback(f)) => {
                matches!(f.gate, Gate::Schema | Gate::Binding)
            }
        };
        if !shareable {
            return SessionAction::Cold;
        }
        let open = self.state.lock().map(|s| s.open).unwrap_or(false);
        if open {
            SessionAction::Resume(self.id)
        } else {
            SessionAction::Start(self.id)
        }
    }

    /// Record a finished call. Called only after the subprocess succeeded, so
    /// a failed start does not leave the session marked open — the next call
    /// would then resume an id the provider never wrote.
    pub(crate) fn record(
        &self,
        step: &str,
        action: SessionAction,
        usage: Option<AgentUsage>,
        model: Option<String>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(action, SessionAction::Start(_)) {
                state.open = true;
            }
            state.calls.push(AgentCallRecord {
                step: step.to_string(),
                resumed: action.resumed(),
                usage,
                model,
            });
        }
    }

    /// The models this window's calls ran on, each once, in the order they
    /// first appeared.
    #[must_use]
    pub fn models(&self) -> Vec<String> {
        let mut models: Vec<String> = Vec::new();
        for call in self.calls() {
            if let Some(model) = call.model {
                if !models.contains(&model) {
                    models.push(model);
                }
            }
        }
        models
    }

    /// What every call in this window was and cost, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<AgentCallRecord> {
        self.state
            .lock()
            .map(|s| s.calls.clone())
            .unwrap_or_default()
    }

    /// The window's total, or `None` when no call reported anything.
    #[must_use]
    pub fn total(&self) -> Option<AgentUsage> {
        let calls = self.calls();
        let reported: Vec<AgentUsage> = calls.iter().filter_map(|c| c.usage).collect();
        if reported.is_empty() {
            return None;
        }
        Some(
            reported
                .into_iter()
                .fold(AgentUsage::default(), |a, b| a.plus(b)),
        )
    }
}

/// Default for `ENRICH_AGENT_IDLE_SECS`. Generous on purpose: the claude
/// provider streams every token, so ten silent minutes means no token at
/// all, not a long answer. API retries after an overload are the slowest
/// legitimate silence, and they stay well under this.
const DEFAULT_AGENT_IDLE_SECS: u64 = 600;

/// The stall limit from `ENRICH_AGENT_IDLE_SECS`: unset or unreadable gives
/// the default, `0` turns the check off.
fn agent_idle_timeout(raw: Option<&str>) -> Option<Duration> {
    let secs = raw
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_AGENT_IDLE_SECS);
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// The label a call is accounted under.
fn pass_label(pass: &Pass) -> &'static str {
    match pass {
        Pass::Translate => "translate",
        Pass::Feedback(f) => f.gate.label(),
    }
}

/// Decide what this call does with the window's session. A payload without a
/// session (document-convert, a test that does not care) is always cold.
fn begin_call(payload: &EnrichPayload) -> SessionAction {
    payload
        .session
        .as_ref()
        .map_or(SessionAction::Cold, |s| s.plan(&payload.pass))
}

/// Record a finished call against the window's session.
fn end_call(payload: &EnrichPayload, action: SessionAction, run: AgentRun) {
    if let Some(session) = payload.session.as_ref() {
        session.record(pass_label(&payload.pass), action, run.usage, run.model);
    }
}

/// What one agent subprocess reported about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgentRun {
    /// Token use and cost, from the closing event.
    pub usage: Option<AgentUsage>,
    /// The model it ran on, from the `init` event.
    pub model: Option<String>,
}

/// Max bytes of the LLM subprocess's stderr to retain for diagnostics. The tail
/// (most recent output) is kept and appended to the error on a non-zero exit, so
/// a failure reports the real cause (e.g. an auth `401`) instead of a bare code.
const MAX_STDERR_CAPTURE: usize = 4096;

/// Max bytes of the agent's stdout to retain, enough to hold the provider's
/// closing JSON object with the usage figures and nothing before it.
const MAX_STDOUT_TAIL: usize = 32 * 1024;

/// Default runner that spawns a real CLI process.
pub struct ProcessLlmRunner;

#[async_trait::async_trait]
impl LlmRunner for ProcessLlmRunner {
    async fn run(
        &self,
        payload: &EnrichPayload,
        yaml_abs: &Path,
        repo_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()> {
        let progress_path = progress_file_path(yaml_abs);
        // Chunked runs get the explicit-article-subset prompt; whole-law runs
        // keep the original prompt byte-identical.
        // A feedback pass asks for one thing and gives the agent nothing
        // else to do, so it cannot wander back into translating.
        let action = begin_call(payload);
        config.activity.begin_call(pass_label(&payload.pass));
        // Computed for both passes, because a feedback round runs in the same
        // runtime as the translation it answers and the prompt tells it so
        // ("You have no network and no search"). A deny list that only applied
        // to the first pass would make that sentence false from round two on.
        let (plan, deny) = chain_plan(repo_path);
        if let Pass::Feedback(feedback) = &payload.pass {
            let prompt =
                build_feedback_prompt(&payload.yaml_path, feedback, vocabulary_of(yaml_abs).await);
            // A resumed round starts from a conversation in which the agent
            // wrote the very thing the gate is complaining about, so it is
            // told first of all to go and look.
            let prompt = if action.resumed() {
                format!("{REREAD_INSTRUCTION}\n\n{prompt}")
            } else {
                prompt
            };
            let run = run_llm_subprocess(
                &config.provider,
                &prompt,
                Some(yaml_abs),
                repo_path,
                config,
                ToolPolicy {
                    allow_bash: false,
                    deny: &deny,
                },
                action,
            )
            .await?;
            end_call(payload, action, run);
            return Ok(());
        }

        // What the chain may instruct depends on what this runtime grants.
        // A step whose tools are absent is left out and recorded, rather than
        // asked for and answered with an invention (issue #1036).
        let report = capabilities::plan_report(&plan);
        if !report.is_empty() {
            tracing::info!(plan = %report.trim_end(), "enrichment chain planned");
        }
        if let Some((spec, _)) = plan
            .iter()
            .find(|(_, p)| matches!(p, capabilities::StepPlan::Blocked { .. }))
        {
            return Err(PipelineError::Config(format!(
                "required step {:?} cannot run in this runtime: {}",
                spec.name,
                report.trim_end()
            )));
        }
        // Requirement 6 of RFC-026: an article never reaches an agent without
        // its place in the structure and without what bears on it inside its
        // own law. The worker assembles that and writes it down; the agent
        // reads a file rather than hunting through the YAML.
        // `ENRICH_CONTEXT_BRIEF=0` withholds the brief. It exists so a round
        // can be run twice over the same laws with only this changed, which is
        // the only way to say what the brief is worth rather than that the
        // numbers moved.
        let brief = if std::env::var("ENRICH_CONTEXT_BRIEF").as_deref() == Ok("0") {
            tracing::info!("context brief withheld by ENRICH_CONTEXT_BRIEF=0");
            None
        } else {
            context::write_brief(yaml_abs, payload.chunk_articles.as_deref(), repo_path)
        };
        if brief.is_none() {
            tracing::warn!(law = %payload.yaml_path, "no context brief written");
        }
        let prompt = build_prompt(
            &payload.yaml_path,
            &progress_path.to_string_lossy(),
            &plan,
            payload.chunk_articles.as_deref(),
            payload.skip_mvt.unwrap_or(false),
            brief.is_some(),
        );
        let run = run_llm_subprocess(
            &config.provider,
            &prompt,
            Some(yaml_abs),
            repo_path,
            config,
            ToolPolicy {
                // Enrich edits YAML in place; it does not need shell access.
                allow_bash: false,
                deny: &deny,
            },
            action,
        )
        .await?;
        end_call(payload, action, run);
        Ok(())
    }
}

/// What tools one agent call may use.
///
/// Two fields because the provider takes two answers and they are not the same
/// question. The allowlist decides what is approved without asking; the deny
/// list decides what is not there at all. Conflating them is how the enrichment
/// lane came to report a shell-less runtime to an agent that had a shell.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ToolPolicy<'a> {
    /// Whether the `claude` provider's allowlist includes `Bash` (enrich keeps
    /// it off; document-convert needs it so the agent can run/install a
    /// converter). No effect on `opencode`, which has its own tool model.
    pub allow_bash: bool,
    /// Tools withheld outright. Unlike leaving a tool off the allowlist, which
    /// only means it has to be asked for, this takes it away.
    pub deny: &'a [String],
}

impl ToolPolicy<'_> {
    /// The claude provider's `--allowedTools` value.
    fn allowed(self) -> &'static str {
        if self.allow_bash {
            "Bash,Read,Edit,Write,Grep,Glob"
        } else {
            "Read,Edit,Write,Grep,Glob"
        }
    }

    /// The claude provider's `--disallowedTools` value, empty when nothing is
    /// withheld. A caller that did not ask for the shell does not merely fail
    /// to get it approved: it does not get it.
    fn denied(self) -> Vec<String> {
        let mut out: Vec<String> = self.deny.to_vec();
        if !self.allow_bash {
            out.push("Bash".to_owned());
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Spawn and supervise an LLM agent subprocess, provider-agnostically.
///
/// This is the reusable core lifted out of [`ProcessLlmRunner::run`]: it builds
/// the command, drains stdout/stderr (retaining a bounded stderr tail for the
/// error message), and races the child against the configured timeout and the
/// RSS memory watchdog, killing the whole process group on either. `cwd` is the
/// working directory the agent runs in (and writes its output into); `file_arg`
/// is the optional single input file (OpenCode's `-f`). Callers supply their own
/// `prompt` — enrich and document-convert differ only in that prompt.
/// `tools` says what this call may use (see [`ToolPolicy`]).
/// `session` says whether this call opens, continues or ignores a session; a
/// caller with no window to speak of passes [`SessionAction::Cold`].
pub(crate) async fn run_llm_subprocess(
    provider: &LlmProvider,
    prompt: &str,
    file_arg: Option<&Path>,
    cwd: &Path,
    config: &EnrichConfig,
    tools: ToolPolicy<'_>,
    session: SessionAction,
) -> Result<AgentRun> {
    let provider_name = provider.name().to_string();

    let mut cmd = build_command(
        provider,
        prompt,
        file_arg,
        cwd,
        tools,
        config.effort.as_deref(),
        session,
    );

    // Both streams are piped and drained. stdout is drained-and-discarded: a
    // verbose agent (e.g. opencode `--format json`) inlines the full body of
    // every fetched page into its event stream, which would flood container
    // logs. stderr is drained too — we MUST keep reading both or a full 64 KB
    // OS pipe buffer blocks the child — but for stderr we also keep a bounded
    // tail and re-log previews, so the LLM's real error (e.g. an auth 401) is
    // both visible in the logs and attached to the job's failure.
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| PipelineError::Enrich(format!("failed to spawn {}: {e}", provider_name)))?;

    // Capture the PID before any wait reaps the child; the memory watchdog
    // and process-group kill both need it.
    let pid = child.id();
    let activity = config.activity.clone();
    activity.enter();
    // However this call ends, it stops counting as in flight.
    let _in_flight = ExitOnDrop(activity.clone());
    // Only the claude provider streams per token, so only there does silence
    // mean the agent stopped. opencode writes an event per finished part and
    // can be quiet for longer while it writes a large file.
    let idle_limit = match provider {
        LlmProvider::Claude { .. } => config.idle_timeout,
        LlmProvider::OpenCode { .. } => None,
    };

    // Drain stderr, retaining the last `MAX_STDERR_CAPTURE` bytes for the
    // error message and re-logging bounded previews so it stays visible.
    let stderr_tail = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let stderr_task = child.stderr.take().map(|mut stderr| {
        let tail = stderr_tail.clone();
        let stderr_provider = provider_name.clone();
        let activity = activity.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break, // EOF or pipe gone
                    Ok(n) => {
                        activity.touch();
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let preview: String = text.trim().chars().take(500).collect();
                        if !preview.is_empty() {
                            tracing::warn!(provider = %stderr_provider, %preview, "agent stderr");
                        }
                        if let Ok(mut t) = tail.lock() {
                            t.push_str(&text);
                            if t.len() > MAX_STDERR_CAPTURE {
                                let mut cut = t.len() - MAX_STDERR_CAPTURE;
                                while cut < t.len() && !t.is_char_boundary(cut) {
                                    cut += 1;
                                }
                                *t = t[cut..].to_string();
                            }
                        }
                    }
                }
            }
        })
    });
    // Read the retained stderr tail, formatted as a "; stderr: …" suffix.
    let stderr_suffix = || {
        stderr_tail
            .lock()
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(|t| format!("; stderr: {t}"))
            .unwrap_or_default()
    };

    // Drain the agent's stdout so it never reaches container logs. We MUST
    // keep reading it: if the OS pipe buffer (64 KB) fills, the child blocks
    // indefinitely — the same deadlock the stderr comment above warns about.
    // The task ends on EOF when the process exits or is killed.
    // Tail of stdout, kept so the provider's closing JSON object can be read
    // after the process exits. Bounded: everything before the last object is
    // of no interest, and the stream can carry megabytes.
    let stdout_tail = std::sync::Arc::new(Mutex::new(String::new()));
    let tail_writer = std::sync::Arc::clone(&stdout_tail);
    // The claude provider's closing `result` event, kept whole: it carries
    // the usage, and the bounded tail can cut a long one in half.
    let result_line = std::sync::Arc::new(Mutex::new(None::<String>));
    let result_writer = std::sync::Arc::clone(&result_line);
    // The model the CLI names in its `init` event, for the call's record.
    let init_model = std::sync::Arc::new(Mutex::new(None::<String>));
    let model_writer = std::sync::Arc::clone(&init_model);
    let mut stdout_task = None;
    if let Some(mut stdout) = child.stdout.take() {
        let drain_provider = provider_name.clone();
        let activity = activity.clone();
        stdout_task = Some(tokio::spawn(async move {
            let mut lines = crate::agent_activity::LineSplitter::default();
            // Drain in fixed-size chunks rather than whole lines: opencode
            // inlines multi-MB page bodies as a single JSON line, so a
            // line reader would allocate the entire body just to log a
            // 200-char preview — a heap spike on the very worker this
            // watchdog exists to protect. Reading into a fixed buffer keeps
            // the pipe empty without ever holding more than `buf`.
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) | Err(_) => break, // EOF or pipe gone (process exited/killed)
                    Ok(n) => {
                        activity.touch();
                        lines.feed(&buf[..n], |line| {
                            activity.record_line(line);
                            if let Some(model) = crate::agent_activity::init_model(line) {
                                tracing::info!(provider = %drain_provider, %model, "agent session started");
                                if let Ok(mut m) = model_writer.lock() {
                                    *m = Some(model);
                                }
                            }
                            if crate::agent_activity::is_result_event(line) {
                                if let Ok(mut r) = result_writer.lock() {
                                    *r = Some(line.to_string());
                                }
                            }
                        });
                        // Bounded preview at debug only (off under the
                        // default "info" subscriber). The leading bytes of a
                        // read carry the event type and ids, not the large
                        // inlined bodies; lossy is fine for a log preview.
                        let preview = String::from_utf8_lossy(&buf[..n.min(200)]);
                        let preview = preview.trim_end();
                        if !preview.is_empty() {
                            tracing::debug!(provider = %drain_provider, %preview, "agent stdout");
                        }
                        #[allow(clippy::unwrap_used)]
                        if let Ok(mut tail) = tail_writer.lock() {
                            tail.push_str(&String::from_utf8_lossy(&buf[..n]));
                            if tail.len() > MAX_STDOUT_TAIL {
                                let cut = tail.len() - MAX_STDOUT_TAIL;
                                let cut = tail
                                    .char_indices()
                                    .map(|(i, _)| i)
                                    .find(|i| *i >= cut)
                                    .unwrap_or(tail.len());
                                *tail = tail.split_off(cut);
                            }
                        }
                    }
                }
            }
        }));
    }

    let status = tokio::select! {
        result = child.wait() => {
            result.map_err(|e| {
                PipelineError::Enrich(format!("failed to wait for {}: {e}", provider_name))
            })?
        }
        _ = tokio::time::sleep(config.timeout) => {
            terminate(&mut child, pid).await;
            // Abort the drain task rather than leaving it detached: if a
            // grandchild inherited fd 2 and survived terminate(), the task
            // would otherwise leak. The tail read below is already populated.
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{} timed out after {:?}{}{}",
                provider_name,
                config.timeout,
                stderr_suffix(),
                activity.summary()
            )));
        }
        silent = wait_for_silence(&activity, idle_limit) => {
            tracing::error!(
                provider = %provider_name,
                silent_secs = silent.as_secs(),
                "LLM subprocess wrote nothing for too long, killing it as stalled"
            );
            terminate(&mut child, pid).await;
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{} stalled: no output for {}s{}{}",
                provider_name,
                silent.as_secs(),
                stderr_suffix(),
                activity.summary()
            )));
        }
        observed_mb = watch_memory(pid, config.max_rss_mb) => {
            tracing::error!(
                provider = %provider_name,
                pid = ?pid,
                observed_mb,
                limit_mb = config.max_rss_mb,
                "LLM subprocess exceeded memory limit, killing to protect the container"
            );
            terminate(&mut child, pid).await;
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{provider_name} exceeded memory limit of {} MB (RSS {observed_mb} MB), killed{}",
                config.max_rss_mb, stderr_suffix()
            )));
        }
    };

    // Let stdout drain to its end so the closing event is in, on success for
    // its usage and on failure for its error text. Bounded: a grandchild
    // that inherited the pipe could keep it open and never EOF, and this
    // await sits outside the timeout/memory select!.
    if let Some(task) = stdout_task {
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }

    if !status.success() {
        // Give the stderr drain a moment to finish so the tail is complete,
        // bounded for the same reason as stdout above. Best-effort, like the
        // tail in the other paths.
        if let Some(task) = stderr_task {
            let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
        }
        // The claude CLI reports a failed API call in its closing event and
        // exits 1 with nothing on stderr; without this the reason is lost.
        let cli_error = result_line
            .lock()
            .ok()
            .and_then(|r| {
                r.as_deref()
                    .and_then(crate::agent_activity::result_error_text)
            })
            .map(|t| format!(": {t}"))
            .unwrap_or_default();
        return Err(PipelineError::Enrich(format!(
            "{} exited with {}{}{}{}",
            provider_name,
            status,
            cli_error,
            stderr_suffix(),
            activity.summary()
        )));
    }

    // Success: abort the drain task instead of leaving it detached — same
    // fd-2/grandchild-leak guard as the timeout/OOM paths. Normally it has
    // already finished (the child closed stderr on exit); aborting a finished
    // task is a no-op.
    if let Some(t) = &stderr_task {
        t.abort();
    }

    let from_result = result_line
        .lock()
        .ok()
        .and_then(|r| r.as_deref().and_then(AgentUsage::from_stdout_tail));
    let usage = from_result.or_else(|| {
        stdout_tail
            .lock()
            .ok()
            .and_then(|tail| AgentUsage::from_stdout_tail(&tail))
    });
    if let Some(u) = usage {
        tracing::info!(
            provider = %provider_name,
            resumed = session.resumed(),
            input_tokens = u.input_tokens,
            output_tokens = u.output_tokens,
            cache_read_tokens = u.cache_read_tokens,
            cost_millicents = u.cost_millicents,
            "agent run accounted"
        );
    }
    let model = init_model.lock().ok().and_then(|m| m.clone());
    Ok(AgentRun { usage, model })
}

/// Marks the agent subprocess as ended when its call returns, by any path.
struct ExitOnDrop(crate::agent_activity::ActivityHandle);

impl Drop for ExitOnDrop {
    fn drop(&mut self) {
        self.0.exit();
    }
}

/// Resolve once the agent has written nothing for `limit`, with how long it
/// has been silent. Never resolves when the check is off.
async fn wait_for_silence(
    activity: &crate::agent_activity::ActivityHandle,
    limit: Option<Duration>,
) -> Duration {
    let Some(limit) = limit else {
        return std::future::pending().await;
    };
    // Check a few times per limit, at most every 15 s, so a short limit in a
    // test still fires promptly.
    let every = (limit / 4).clamp(Duration::from_millis(50), Duration::from_secs(15));
    loop {
        tokio::time::sleep(every).await;
        if let Some(silent) = activity.idle() {
            if silent >= limit {
                return silent;
            }
        }
    }
}

/// Kill the LLM subprocess and reap it.
///
/// Signals the whole process group (negative pid) so any helpers the agent
/// forked (node workers, git) die too — not just the direct child — then
/// falls back to `child.kill()` (covers a missing pid) and waits to avoid a
/// zombie.
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    kill_process_group(pid);
    if let Err(e) = child.kill().await {
        // After the group SIGKILL above the direct child is usually already
        // gone, so `ESRCH` here is the expected benign race — only a different
        // error is worth flagging.
        if e.raw_os_error() != Some(libc::ESRCH) {
            tracing::warn!(error = %e, "failed to kill LLM process");
        }
    }
    let _ = child.wait().await;
}

/// Send `SIGKILL` to the entire process group led by `pid`.
///
/// The subprocess is spawned as its own process-group leader
/// (`process_group(0)` in `build_command`), so `kill(-pid, …)` reaps the agent
/// and everything it forked. A failure (e.g. `ESRCH`) just means the group has
/// already exited and is ignored.
fn kill_process_group(pid: Option<u32>) {
    if let Some(pid) = pid {
        // Guard against pid 0: `kill(-0, …)` collapses to `kill(0, …)`, which
        // POSIX routes to the *caller's own* process group — it would SIGKILL
        // the worker itself. `child.id()` never yields 0 in practice, but the
        // consequence is catastrophic enough to refuse it defensively.
        if pid == 0 {
            return;
        }
        // SAFETY: `kill(2)` with a negative pid targets a process group and has
        // no memory-safety implications; the return value is intentionally
        // ignored (the group may already be gone). Linux PIDs are capped well
        // below `i32::MAX` (`pid_max` <= 2^22), so `pid as i32` cannot overflow.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

/// Memory watchdog: resolve with the observed RSS (in MB) once the subprocess
/// exceeds `max_rss_mb`.
///
/// The LLM agent accumulates fetched-page bodies in memory across a run; left
/// unchecked it climbs to the container's cgroup limit and triggers an OOM kill
/// of the whole pod. Polling RSS and killing the offender instead turns that
/// into a clean, retryable job failure.
///
/// Disabled (never resolves, letting the `child.wait()`/timeout branches win)
/// when `pid` is `None` or `max_rss_mb` is 0. We poll `/proc` directly rather
/// than capping virtual memory with `RLIMIT_AS`: V8 reserves a huge virtual
/// address space, so a virtual-memory ceiling would crash Node outright.
async fn watch_memory(pid: Option<u32>, max_rss_mb: u64) -> u64 {
    let pid = match pid {
        Some(p) if max_rss_mb > 0 => p,
        _ => {
            tracing::debug!("enrich memory watchdog disabled (no pid or zero limit)");
            std::future::pending::<()>().await;
            unreachable!("pending future never resolves");
        }
    };

    // RSS polling reads `/proc/<pid>/status`, which only exists on Linux (the
    // deploy target). On a non-Linux dev machine `read_vmrss_kb` would return
    // `None` every poll and the watchdog would never fire — silently. Make that
    // degradation explicit and skip the pointless poll loop rather than letting
    // a developer believe a ceiling is enforced when it is not.
    if !cfg!(target_os = "linux") {
        tracing::debug!(
            "enrich memory watchdog inactive: RSS polling needs /proc (Linux only); \
             agent runs without a memory ceiling on this platform"
        );
        std::future::pending::<()>().await;
        unreachable!("pending future never resolves");
    }

    let interval = Duration::from_secs(5);
    loop {
        tokio::time::sleep(interval).await;
        // A missing/unparsable status file means the process is gone; keep
        // looping (harmless) and let `child.wait()` win the select.
        if let Some(rss_kb) = read_vmrss_kb(pid).await {
            let rss_mb = rss_kb / 1024;
            if rss_mb > max_rss_mb {
                return rss_mb;
            }
        }
    }
}

/// Read a process's resident set size (RSS) from `/proc/<pid>/status`, in kB.
/// Returns `None` if the file is missing or unparsable (e.g. the process exited).
///
/// This measures only the direct child PID, not the whole process group that
/// `kill_process_group` SIGKILLs. That asymmetry is intentional: for the current
/// opencode/Claude agent topology the runaway memory is the accumulated
/// fetched-page bodies held in the main node process we spawn, so its RSS is the
/// signal that matters. If a future agent moved that growth into a forked
/// subprocess, this would need to sum the group's RSS (e.g. walk
/// `/proc/<pid>/task` / children) to stay accurate.
async fn read_vmrss_kb(pid: u32) -> Option<u64> {
    let status = tokio::fs::read_to_string(format!("/proc/{pid}/status"))
        .await
        .ok()?;
    parse_vmrss_kb(&status)
}

/// Parse the `VmRSS` value (in kB) out of `/proc/<pid>/status` contents.
fn parse_vmrss_kb(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb| kb.parse::<u64>().ok())
}

/// What kind of run this is. A chain is a sequence of passes over the same
/// law, and a gate between two of them decides whether the next pass is a
/// translation or a response to what the gate found.
///
/// Modelled as data rather than as a set of optional fields on the payload,
/// because the number of gates grows and each one would otherwise add
/// another flag nobody can see the shape of. RFC-028 generalises this into a
/// step with its own runtime and model; this is the domain half of that,
/// without the machinery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Pass {
    /// Translate: read the law and write `machine_readable`.
    #[default]
    Translate,
    /// Respond to what a gate found. The artefact exists; this pass is about
    /// what is wrong with it.
    Feedback(Feedback),
}

/// What a gate found and what the agent may do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub gate: Gate,
    /// The findings, verbatim. Never paraphrased: an instance path is what
    /// says where the problem sits, and the agent cannot run the check
    /// itself to look again.
    pub findings: Vec<String>,
}

/// The gate that produced the findings. Its kind decides what an acceptable
/// answer is, which is the difference that matters most in this design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// A fact about the artefact that must hold: the YAML validates. Not
    /// open to interpretation, so the only answers are repair or stop.
    Schema,
    /// A question the statutory text raises: this lid says "in afwijking
    /// van" and the model has no branch; this article computes over a
    /// berekeningsjaar and no binding carries a period. The answer may be a
    /// change to the model, and it may equally be a marking that says why
    /// not. A gate that only accepts changes turns every open norm into a
    /// defect and pushes the translator into inventing something.
    Checks,
    /// The record rather than the translation: a marking filed in the wrong
    /// drawer, or a source named that the agent had no way of reading. Soft,
    /// because a misfiled marking is still better than silence, and because
    /// failing here would punish the very behaviour this design wants.
    Marking,
    /// The closing pass over the whole law, once the last window has been
    /// walked. Not a window gate: it exists because a window cannot bind to an
    /// output that did not exist while it ran, so part of what stands is a
    /// measurement taken too early rather than a defect. Its findings are the
    /// leads [`crate::enrich_v2::reconcile`] refuses to resolve on its own,
    /// and the pass that answers them may connect and nothing else.
    Reconcile,
    /// A fact about the whole law once its last window has been walked: every
    /// intra-law reference resolves ([`crate::enrich_v2::checks::dangling_bindings`]).
    /// Hard like [`Gate::Schema`], because a marking is no answer to "this
    /// variable does not exist": the law is schema-valid and fails the moment
    /// it runs.
    ///
    /// Not a window gate, for the reason [`Gate::Reconcile`] is not one. A
    /// window may read an output a later window has yet to write, so the same
    /// finding under [`Gate::Checks`] is a measurement taken too early and
    /// stays soft there. Only after the closing pass has connected what it
    /// could is an unresolved reference a defect of the law.
    Binding,
}

impl Gate {
    /// Whether a marking is an acceptable answer instead of a change.
    fn accepts_marking(self) -> bool {
        matches!(self, Gate::Checks | Gate::Marking | Gate::Reconcile)
    }

    /// Stable lowercase name, used in logs, in the measurement record and as
    /// the key of a per-gate round budget.
    pub fn label(self) -> &'static str {
        match self {
            Gate::Schema => "schema",
            Gate::Checks => "checks",
            Gate::Marking => "marking",
            Gate::Reconcile => "reconcile",
            Gate::Binding => "binding",
        }
    }

    /// The gates in the order the worker runs them for one window. The
    /// closing gates are not among them: they run once, over the whole law,
    /// after the last window.
    pub const ALL: [Gate; 3] = [Gate::Schema, Gate::Checks, Gate::Marking];
}

/// The most agent calls one enrichment run can make, end to end.
///
/// One translation pass, one feedback round per gate, the closing pass, the
/// final binding gate and the final schema gate, with the default one round
/// per gate; the worker sizes the per-call timeouts from
/// [`FeedbackRounds::max_calls`], which follows the configured rounds.
/// Bounding one call against the whole job budget assumes a run is
/// one call, which it stopped being when the gates gained a feedback round.
/// With the defaults that assumption let a run ask for six times 600 s under
/// a 1200 s ceiling, so a law needing more than two calls could never finish
/// and went to `enrich_failed` on every retry.
///
/// A ceiling, not a reservation: a clean gate spends no round at all, and
/// most runs make far fewer calls than this.
pub const MAX_AGENT_CALLS_PER_RUN: u32 = 1 + Gate::ALL.len() as u32 + 1 + 1 + 1;

/// How many feedback rounds each gate may run.
///
/// One per gate rather than one number for the whole chain: the open question
/// is not whether a second round helps, it is which gate it helps at. A schema
/// error either gets repaired or does not; a coverage question may genuinely
/// take a second look. Defaults to one round everywhere, which is the
/// behaviour that existed before this was settable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackRounds {
    pub schema: usize,
    pub checks: usize,
    pub marking: usize,
    pub reconcile: usize,
    pub binding: usize,
}

impl Default for FeedbackRounds {
    fn default() -> Self {
        Self::uniform(1)
    }
}

impl FeedbackRounds {
    /// The same budget for every gate.
    #[must_use]
    pub fn uniform(rounds: usize) -> Self {
        Self {
            schema: rounds,
            checks: rounds,
            marking: rounds,
            reconcile: rounds,
            binding: rounds,
        }
    }

    /// The most feedback rounds one run can spend over all gates: each window
    /// gate, the closing pass, the binding gate, and the schema gate a second
    /// time after the closing pass. With one round per gate this is
    /// [`MAX_AGENT_CALLS_PER_RUN`] minus the translation.
    #[must_use]
    pub fn max_calls(self) -> u32 {
        let total = self
            .schema
            .saturating_mul(2)
            .saturating_add(self.checks)
            .saturating_add(self.marking)
            .saturating_add(self.reconcile)
            .saturating_add(self.binding);
        u32::try_from(total).unwrap_or(u32::MAX)
    }

    /// Budget for one gate.
    #[must_use]
    pub fn for_gate(self, gate: Gate) -> usize {
        match gate {
            Gate::Schema => self.schema,
            Gate::Checks => self.checks,
            Gate::Marking => self.marking,
            Gate::Reconcile => self.reconcile,
            Gate::Binding => self.binding,
        }
    }

    /// Parse a budget spec: either a bare number for every gate (`2`) or a
    /// comma-separated list of per-gate overrides on top of the default
    /// (`checks=2,marking=3`). The two forms combine — `2,schema=1` gives
    /// every gate two rounds except schema.
    pub fn parse(spec: &str) -> std::result::Result<Self, String> {
        let mut rounds = Self::default();
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                None => {
                    let n = part
                        .parse::<usize>()
                        .map_err(|_| format!("not a number of rounds: {part}"))?;
                    rounds = Self::uniform(n);
                }
                Some((gate, value)) => {
                    let n = value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| format!("not a number of rounds: {value}"))?;
                    match gate.trim() {
                        "schema" => rounds.schema = n,
                        "checks" => rounds.checks = n,
                        "marking" => rounds.marking = n,
                        "reconcile" => rounds.reconcile = n,
                        "binding" => rounds.binding = n,
                        other => return Err(format!("unknown gate: {other}")),
                    }
                }
            }
        }
        Ok(rounds)
    }
}

/// Why a gate stopped running rounds. Recorded per round so the measurement
/// says what ended the chain, not only how long it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoundStop {
    /// Nothing left to say: the gate reports no findings.
    Cleared,
    /// The agent left the file byte-identical, so nothing it did can have
    /// removed a finding and nothing suggests the next round differs.
    Unchanged,
    /// The file changed but the finding count did not fall. Either churn or a
    /// trade, and in both cases another round has no evidence behind it.
    NoDecrease,
    /// The configured number of rounds is used up.
    Budget,
}

/// What one feedback round did to one gate.
///
/// Findings and markings sit side by side on purpose. A round can lower the
/// finding count by translating better and it can lower it by declaring more
/// of the article unmodellable, and those are opposite outcomes. Reporting
/// only the findings would make the second look like the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackRoundRecord {
    /// 1-based round number within this gate.
    pub round: usize,
    pub findings_before: usize,
    pub findings_after: usize,
    /// Markings in the file before/after this round, or `None` when the law
    /// could not be parsed at that moment (a schema round may start on a file
    /// that does not load).
    pub markings_before: Option<usize>,
    pub markings_after: Option<usize>,
    /// Whether the agent changed the file at all during this round.
    pub file_changed: bool,
    /// Set on the last round of the gate; `None` while more rounds follow.
    pub stopped: Option<RoundStop>,
}

/// The feedback rounds one gate ran, in order. An empty `rounds` means the
/// gate had nothing to say and no agent was asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateFeedback {
    /// `schema`, `checks` or `marking` — see [`Gate::label`].
    pub gate: String,
    /// Findings when the gate was first evaluated, before any round.
    pub findings_initial: usize,
    /// Findings left after the last round (equal to `findings_initial` when
    /// no round ran).
    pub findings_final: usize,
    pub rounds: Vec<FeedbackRoundRecord>,
    /// Findings this gate saw that no edit to this file could answer: a
    /// binding onto a law the corpus does not have, or has and has not yet
    /// interpreted. Never put to the agent, because the answer is a harvest or
    /// an interpretation elsewhere, and recorded here because it is still work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outside_corpus: Vec<String>,
}

/// Payload for an enrich job, stored as JSON in the job queue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichPayload {
    pub law_id: String,
    /// Relative path to the harvested YAML file within the repo.
    pub yaml_path: String,
    /// LLM provider to use for this enrichment ("opencode" or "claude").
    /// When set, overrides the worker's `LLM_PROVIDER` env var.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Recursion depth for related-legislation follow-up harvests. Inherited
    /// from the harvest job that spawned this enrichment. `None` or `0` means a
    /// root enrichment; the child harvests it enqueues get `depth + 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// Account dat de taak-flow-enrichment aanvroeg (gezet wanneer
    /// `deliver == "task"`); bepaalt de assignee van de review-taak.
    /// De taak-flow-gate zelf is `deliver_as_task()`, niet dit veld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
    /// `"task"` ⇒ resultaat als job_blobs + taak, géén push (taak-flow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
    /// Eigenaar-traject van de taak-flow (voor de tasks-rij + save-URL's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traject_id: Option<Uuid>,
    /// URL-vorm van het traject (`{slug}-{8hex}`), voor de task-payload
    /// zodat de frontend er review-URL's mee kan bouwen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traject_ref: Option<String>,
    /// `document_etag()` van de wet-YAML op aanvraagmoment (staleness-check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_etag: Option<String>,
    /// Welke soort run dit is. Rijdt niet mee in de jobpayload: hij bestaat
    /// alleen binnen één uitvoering en zegt wat de agent nu geacht wordt te
    /// doen. Zie [`Pass`].
    #[serde(skip)]
    pub pass: Pass,
    /// `true` wanneer deze enrichment een NIEUWE wet betreft die nog niet in
    /// het traject bestaat (geketend vanuit een `law_convert`-job). Stuurt de
    /// review-taak: `kind: "law_create"` + eigen titel, zodat de editor het
    /// voorstel als aan-te-maken wet behandelt i.p.v. als wijziging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_law: Option<bool>,
    /// Article numbers this run must process (chunked enrichment). Computed
    /// per run by `execute_enrich_with_runner` from the persisted cursor and
    /// passed to the [`LlmRunner`] via the normalized payload — never stored
    /// in queue payloads (`skip_serializing_if` keeps old payloads and the
    /// runner trait untouched). `None` means whole-law mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_articles: Option<Vec<String>>,
    /// De agent-sessie van dít venster: gedeeld door de vertaalslag en de
    /// terugkoppelrondes erna, en samen met het venster afgelopen. Rijdt net
    /// als [`Pass`] niet mee in de jobpayload — hij bestaat alleen binnen één
    /// uitvoering, en een sessie-id dat een wachtrij overleeft zou een gesprek
    /// heropenen dat nergens meer bij hoort. `None` betekent koud: elke
    /// aanroep een eigen proces.
    #[serde(skip)]
    pub session: Option<std::sync::Arc<AgentSession>>,
    /// `true` on continuation chunks (cursor > 0): the MvT-research step ran
    /// during the first chunk and its feature file is already on the branch,
    /// so the prompt tells the agent to skip step 1. Transport-only, like
    /// `chunk_articles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_mvt: Option<bool>,
}

impl EnrichPayload {
    /// Taak-flow: resultaat naar Postgres + taak i.p.v. push naar git.
    pub fn deliver_as_task(&self) -> bool {
        self.deliver.as_deref() == Some("task")
    }

    /// Contract-guard voor het corpus-brede (klassieke) enrich-pad: dat pad
    /// pusht met het centrale corpus-token (`CorpusConfig`/`CORPUS_GIT_TOKEN`)
    /// naar de centrale corpus-repo — de operator-repo — en mag dus alléén
    /// corpus-brede jobs verwerken. Een payload met een traject-doel
    /// (`traject_id`/`traject_ref`) hoort bij de taak-flow
    /// (`deliver: "task"` → blob + review-taak; zie het worker/traject-contract
    /// in de crate-doc); belandt zo'n payload tóch hier, dan is dat een
    /// enqueue-fout die terminaal en luid moet falen in plaats van met het
    /// server-token naar een verkeerd doel te schrijven.
    pub fn require_corpus_wide_target(&self) -> Result<()> {
        if self.traject_id.is_some() || self.traject_ref.is_some() {
            return Err(PipelineError::Worker(
                "enrich-payload met traject-doel zonder deliver=task: traject-oplevering \
                 loopt altijd via een review-taak, het corpus-brede push-pad is alleen \
                 voor de centrale corpus-repo"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// All known provider names. Used to create one enrich job per provider
/// after a successful harvest.
pub const ENRICH_PROVIDERS: &[&str] = &["opencode", "claude"];

/// A related-legislation reference returned by the enrichment agent in the
/// `.enrichment-result.yaml` sidecar (the "result envelope").
///
/// The extref-only recursive harvester only follows explicit BWB cross-links in
/// the source text, so it misses delegated regelingen and other laws a
/// machine-readable model actually depends on (a `source.regulation`, a
/// `legal_basis`, or an `open_term` delegation). The enrichment agent knows
/// these because it just modeled them, so it declares them here and the worker
/// enqueues follow-up harvests — letting the dependency graph fill itself in.
///
/// This lives OUTSIDE the law schema on purpose: the law YAML stays
/// schema-conformant, and this provenance/routing metadata rides alongside it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelatedLegislation {
    /// Human-readable name of the related law/regeling (used for SRU fallback
    /// resolution when no `bwb_id`/`slug` is supplied).
    pub name: String,
    /// How this law relates: `source_regulation`, `legal_basis`, or
    /// `delegated_regeling`. Informational; the worker treats all the same.
    #[serde(default)]
    pub relation: String,
    /// Best-effort BWB identifier (e.g. "BWBR0018451"). Preferred resolution.
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// Best-effort corpus slug (e.g. "wet_op_de_zorgtoeslag"). Second-choice
    /// resolution, looked up against `law_entries`.
    #[serde(default)]
    pub slug: Option<String>,
    /// The `open_term` id this delegation fills, when `relation` is a delegation.
    #[serde(default)]
    pub open_term: Option<String>,
}

/// The `.enrichment-result.yaml` result envelope written next to an enriched
/// law YAML. Deliberately NOT a law-schema change — see [`RelatedLegislation`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EnrichmentResultEnvelope {
    #[serde(default)]
    pub law_id: Option<String>,
    #[serde(default)]
    pub related_legislation: Vec<RelatedLegislation>,
    /// Per-chunk review report (chunked enrichment only). A chunk may
    /// legitimately add zero `machine_readable` sections (e.g. a
    /// transitional-law chapter); this report is the agent's proof that it
    /// actually reviewed the window, so the run can still count as progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_report: Option<ChunkReport>,
}

/// Proof-of-review for one enrichment chunk, written by the agent into the
/// `.enrichment-result.yaml` envelope. See [`EnrichmentResultEnvelope`].
///
/// Only counts as proof when it references at least one article of the chunk
/// window it was written for (checked by the no-op guard in
/// `execute_enrich_with_runner`): an empty or unrelated report must not
/// advance the cursor past an unreviewed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChunkReport {
    /// Article numbers the agent reviewed this session.
    #[serde(default)]
    pub articles_reviewed: Vec<String>,
    /// Articles deliberately left without `machine_readable`, with the reason
    /// (e.g. "definition article", "transitional law").
    #[serde(default)]
    pub articles_skipped: Vec<SkippedArticle>,
}

/// One deliberately-skipped article in a [`ChunkReport`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkippedArticle {
    pub number: String,
    #[serde(default)]
    pub reason: String,
}

/// Read the sibling `.enrichment-result.yaml` result envelope for a law YAML.
///
/// Never errors, so it can never fail an otherwise-successful enrichment:
/// - absent file → default (empty) envelope;
/// - unparseable file → logged at `warn` and default envelope.
async fn read_enrichment_result_envelope(yaml_abs: &Path) -> EnrichmentResultEnvelope {
    let envelope_path = enrichment_result_path(yaml_abs);
    let content = match tokio::fs::read_to_string(&envelope_path).await {
        Ok(c) => c,
        Err(_) => return EnrichmentResultEnvelope::default(),
    };
    match serde_yaml_ng::from_str::<EnrichmentResultEnvelope>(&content) {
        Ok(envelope) => envelope,
        Err(e) => {
            tracing::warn!(
                path = %envelope_path.display(),
                error = %e,
                "failed to parse .enrichment-result.yaml; ignoring its contents"
            );
            EnrichmentResultEnvelope::default()
        }
    }
}

/// Strip a stale `chunk_report` from the `.enrichment-result.yaml` sidecar
/// before a chunked LLM run.
///
/// The envelope is committed to the enrich branch as provenance, so the fresh
/// checkout of a continuation chunk still contains the *previous* chunk's
/// `chunk_report`. Left in place, the no-op guard would accept that stale
/// report as proof-of-review for THIS window and silently advance the cursor
/// past an unreviewed window. Removing only the `chunk_report` key (all other
/// envelope contents — e.g. `related_legislation` — stay intact, via a raw
/// `Value` edit so unknown keys survive too) guarantees any report present
/// after the run was written this session.
///
/// Best-effort: an absent, unparseable, or non-mapping file is left alone —
/// `read_enrichment_result_envelope` already degrades those to an empty
/// envelope (no `chunk_report`) after the run, which keeps the guard sound.
async fn clear_stale_chunk_report(yaml_abs: &Path) {
    let envelope_path = enrichment_result_path(yaml_abs);
    let Ok(content) = tokio::fs::read_to_string(&envelope_path).await else {
        return;
    };
    let Ok(mut value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content) else {
        return;
    };
    let Some(map) = value.as_mapping_mut() else {
        return;
    };
    if map.remove("chunk_report").is_none() {
        return;
    }
    match serde_yaml_ng::to_string(&value) {
        Ok(stripped) => {
            if let Err(e) = tokio::fs::write(&envelope_path, stripped).await {
                tracing::warn!(
                    path = %envelope_path.display(),
                    error = %e,
                    "failed to strip stale chunk_report from result envelope"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                path = %envelope_path.display(),
                error = %e,
                "failed to re-serialize result envelope while stripping stale chunk_report"
            );
        }
    }
}

/// Path of the `.enrichment-result.yaml` sidecar next to a law YAML file.
fn enrichment_result_path(yaml_abs: &Path) -> PathBuf {
    yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment-result.yaml")
}

/// Result of a successful enrichment execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichResult {
    pub law_id: String,
    pub yaml_path: String,
    pub articles_total: usize,
    /// Total articles with a `machine_readable` section after enrichment
    /// (includes pre-existing ones). Not the count of newly enriched articles.
    pub articles_with_machine_readable: usize,
    /// Fraction of previously-unenriched articles that the LLM enriched
    /// in this session. 1.0 means every article that was missing a
    /// `machine_readable` section now has one; says nothing about correctness.
    pub coverage_score: f64,
    pub provider: String,
    pub branch: String,
    /// Related legislation the enrichment agent declared this law depends on,
    /// read from the `.enrichment-result.yaml` sidecar. The worker uses these to
    /// enqueue follow-up harvests. Empty when no sidecar was written.
    #[serde(default)]
    pub related_legislation: Vec<RelatedLegislation>,
    /// Untranslatable constructs captured from the enriched YAML (RFC-012):
    /// legal constructs the agent could not express with the engine's current
    /// operation set. The worker persists these to the `untranslatables` table;
    /// they also ride here in `jobs.result`. `#[serde(default)]` keeps older
    /// stored results deserializable.
    #[serde(default)]
    pub untranslatables: Vec<CapturedUntranslatable>,
    /// Markings captured from the enriched YAML (schema v0.7.0), the channel
    /// that replaced `untranslatables`. These ride in `jobs.result` and are
    /// deliberately NOT mirrored into the `untranslatables` table: that table
    /// belongs to the v1 pipeline, its `reason` column is `NOT NULL` and a
    /// marking has no such field, so mirroring one would mean inventing the
    /// text a reader is supposed to trust. Where markings land is decided with
    /// the rest of the v2 persistence; until then they are visible in the job
    /// result and counted as progress by the chunk guard.
    #[serde(default)]
    pub markings: Vec<CapturedMarking>,
    /// `false` when this run was a chunk that did NOT reach the end of the
    /// document: the law must stay `enriching` and the worker enqueues a
    /// continuation job. Defaults to `true` so pre-chunking `jobs.result` JSON
    /// (which lacks the field and always covered the whole law) still
    /// deserializes as complete.
    #[serde(default = "default_law_complete")]
    pub law_complete: bool,
    /// Cursor after this run (index of the first unprocessed article, in
    /// document order). 0 in whole-law mode.
    #[serde(default)]
    pub enrich_cursor: usize,
    /// What the feedback rounds did, per gate and per round. Empty when this
    /// run changed nothing and the gates were therefore not run. This is the
    /// measurement: how much each gate's first round takes away and how much
    /// a second still does, with the marking counts beside the findings so a
    /// drop bought by marking more is not read as a drop bought by
    /// translating better.
    #[serde(default)]
    pub feedback: Vec<GateFeedback>,
    /// What this window cost in total, as the provider reported it. `None`
    /// when nothing was reported (opencode, a fake runner, a run with no LLM
    /// call at all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AgentUsage>,
    /// The same figure per call, with the step it belongs to and whether it
    /// continued the window's session. This is what answers whether sharing
    /// the session paid: a resumed round beside a cold one, in the same
    /// window, on the same law.
    #[serde(default)]
    pub agent_calls: Vec<AgentCallRecord>,
    /// The models the window's calls ran on, in the order they first
    /// appeared: usually one. Read from the CLI, so it says what ran even
    /// when the worker set no model and the CLI chose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    /// Which session mode the window ran under (`off`, `repair`, `window`),
    /// so a stored result says what produced its figures.
    #[serde(default)]
    pub session_reuse: String,
}

/// Serde default for [`EnrichResult::law_complete`]: results stored before
/// chunking existed always covered the whole law.
fn default_law_complete() -> bool {
    true
}

/// A single untranslatable captured from an enriched article, flattened for
/// persistence. DB-free by design: it rides in `jobs.result` JSON and is written
/// to the `untranslatables` table by the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedUntranslatable {
    /// The owning article's number (`Article.number`).
    pub article: String,
    pub construct: String,
    pub reason: String,
    pub suggestion: Option<String>,
    pub legal_text_excerpt: Option<String>,
    pub accepted: bool,
}

/// A single marking captured from an enriched article, flattened for
/// reporting. The counterpart of [`CapturedUntranslatable`] for schema v0.7.0
/// and later; see [`EnrichResult::markings`] for why it is not written to the
/// `untranslatables` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedMarking {
    /// The owning article's number (`Article.number`).
    pub article: String,
    /// The construct the format cannot express, in the article's own words.
    pub about: String,
    /// `operation` (the operation must be built) or `model` (the format has no
    /// shape for this construct).
    pub resolution: String,
    pub resolved_by: Option<String>,
    /// The values in this article that cannot be produced. Empty says the
    /// article stays executable.
    pub target: Vec<String>,
    pub legal_text_excerpt: String,
    pub accepted: bool,
}

/// Metadata written alongside the enriched law YAML as `.enrichment.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentMetadata {
    pub law_id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub prompt_hash: String,
    pub code_commit: String,
    pub coverage_score: f64,
    pub articles_total: usize,
    /// Total articles with a `machine_readable` section after enrichment.
    pub articles_with_machine_readable: usize,
    /// Git blob SHA of the base-branch law YAML this enrichment was generated
    /// from. Empty when unknown (files written before this field existed).
    #[serde(default)]
    pub source_hash: String,
    /// Chunked-enrichment cursor: index (document order) of the first article
    /// NOT yet processed by the chunk loop. 0 for legacy files (serde default)
    /// and whole-law runs; equal to `articles_total` once the loop finished.
    #[serde(default)]
    pub enrich_cursor: usize,
    /// The normalized law YAML path the cursor was recorded for. The cursor
    /// only applies when this matches the current target path — a new law
    /// version lives at a different path, which resets the cursor to 0.
    #[serde(default)]
    pub enrich_cursor_path: String,
    /// The window mode the cursor was recorded under. In `document` mode it
    /// counts entries, in `layer` mode it counts layers, and reading one as
    /// the other would silently skip or repeat work — so a change of mode
    /// resets the walk, the same way a change of path does.
    #[serde(default)]
    pub enrich_cursor_mode: String,
}

/// Supported LLM providers for enrichment.
///
/// Both providers manage their own authentication:
/// - **OpenCode/VLAM**: reads `~/.local/share/opencode/auth.json` (set via `opencode auth`)
/// - **Claude**: authenticates with a **personal Claude subscription** via
///   `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`), read directly from the
///   environment; no credentials file is written. `ANTHROPIC_API_KEY` is intentionally NOT
///   used — it is not on `LLM_ENV_ALLOWLIST`, so it is never forwarded to `claude` and can
///   never take precedence over the OAuth token.
///
/// In Docker, set the appropriate env var (forwarded to the subprocess via
/// `LLM_ENV_ALLOWLIST`).
#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenCode {
        path: PathBuf,
        model: Option<String>,
    },
    Claude {
        path: PathBuf,
        model: Option<String>,
    },
}

impl LlmProvider {
    /// Short name used in branch names and metadata.
    pub fn name(&self) -> &str {
        match self {
            LlmProvider::OpenCode { .. } => "opencode",
            LlmProvider::Claude { .. } => "claude",
        }
    }

    /// Model string for metadata (provider-specific default if not set).
    pub fn model_str(&self) -> String {
        match self {
            LlmProvider::OpenCode { model, .. } => {
                model.clone().unwrap_or_else(|| "default".into())
            }
            LlmProvider::Claude { model, .. } => model.clone().unwrap_or_else(|| "default".into()),
        }
    }
}

/// Configuration for enrichment execution.
///
/// All env vars are read once at startup and stored. `with_provider_override()`
/// Which steps of one run to perform.
///
/// Named rather than boolean-per-call because a step is separately useful: the
/// closing reconcile over a law that other windows already finished needs no
/// window of its own, and asking for it should not mean re-running work that
/// is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSteps {
    /// Translate the window and run the gates over it.
    pub window: bool,
    /// The closing pass that connects what now exists.
    pub reconcile: bool,
}

impl RunSteps {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            window: true,
            reconcile: true,
        }
    }

    /// Parse a comma-separated list of step names.
    ///
    /// An unknown name is an error rather than a silent skip: a run that
    /// performs fewer steps than asked, without saying so, is the failure this
    /// codebase spends most of its checks on.
    pub fn parse(spec: &str) -> std::result::Result<Self, String> {
        let mut steps = Self {
            window: false,
            reconcile: false,
        };
        for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match name {
                "window" => steps.window = true,
                "reconcile" => steps.reconcile = true,
                "all" => steps = Self::all(),
                other => return Err(format!("unknown step: {other} (window, reconcile, all)")),
            }
        }
        if !steps.window && !steps.reconcile {
            return Err("no steps selected".to_string());
        }
        Ok(steps)
    }
}

/// selects from pre-built provider configs without re-reading the environment.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    pub provider: LlmProvider,
    /// Ceiling for one agent call outside the feedback rounds: the
    /// translation pass that reads the window and writes `machine_readable`,
    /// by far the heaviest call of a run (and the convert jobs, which set it
    /// themselves).
    pub timeout: Duration,
    /// Ceiling for one feedback round (a gate, the closing pass). Starts equal
    /// to [`Self::timeout`]; the worker lowers the two separately to fit the
    /// job budget, see `bound_llm_timeout`.
    pub feedback_timeout: Duration,
    pub code_commit: String,
    /// RSS ceiling (MB) for the LLM subprocess. When it is exceeded the worker
    /// kills the process and fails the job instead of letting the agent OOM the
    /// whole container. 0 disables the watchdog.
    pub max_rss_mb: u64,
    /// Max articles one enrich run may process (`ENRICH_MAX_ARTICLES_PER_RUN`).
    /// 0 disables chunking (whole-law sessions, the pre-chunking behavior).
    /// Default 15: a 600s session empirically enriches ~5–20 articles and the
    /// law-generate skill batches internally per ~15, so one chunk ≈ one
    /// skill batch.
    pub max_articles_per_run: usize,
    /// Enrich exactly this entry, by number, instead of the window the cursor
    /// points at. Sits beside `max_articles_per_run` because it answers the
    /// same question — which entries this run may touch — and it wins when
    /// set. The run fails when the law has no such entry; it leaves the
    /// cursor where it was, because a repair is not progress through the
    /// document.
    pub target_articles: Vec<String>,
    /// Feedback rounds per gate. Default one everywhere.
    pub feedback_rounds: FeedbackRounds,
    /// What a window is (`ENRICH_WINDOW_MODE`). See [`WindowMode`]; default
    /// `document`, which is the behaviour every run so far has had.
    pub window_mode: WindowMode,
    /// How many windows one run may walk at the same time
    /// (`ENRICH_WINDOW_CONCURRENCY`). Default 1, so nothing shifts under a
    /// measurement that is still running.
    ///
    /// The knob sits between tokens and wall clock. One agent for a whole
    /// layer pays the fixed per-session cost once — window 3 of the running
    /// round cost $5.92 over four calls, of which 5.3 million tokens were
    /// cache reads of the skills, the schema and the context brief. Cutting
    /// that layer into one agent per article multiplies exactly that fixed
    /// cost. Tokens are the scarce thing today; wall clock is the scarce
    /// thing at four thousand laws, which is why this is a setting and not a
    /// design choice.
    pub window_concurrency: usize,
    /// Which steps of the run to perform (`ENRICH_STEPS`), default all.
    ///
    /// The steps are named because they are separately useful. The closing
    /// reconcile used to hang off `law_complete` as the tail of a window, which
    /// made it unreachable for a law that was already finished: the only way to
    /// reach it was to re-enrich a window with nothing left to do, and the
    /// first attempt cost $4.62 to remove one finding. What that pass looks for
    /// was left behind by earlier windows, so it needs no window of its own.
    pub steps: RunSteps,
    /// Whether the calls in one window share a session
    /// (`ENRICH_SESSION_REUSE`). See [`SessionReuse`].
    pub session_reuse: SessionReuse,
    /// Reasoning effort handed to the provider (`claude --effort`: low,
    /// medium, high, xhigh, max). `None` leaves the provider's own default,
    /// which is what every run did before this existed.
    pub effort: Option<String>,
    /// What the agent is doing in the current job. The worker gives each job
    /// a fresh handle and reads it for `jobs.progress`; every clone of this
    /// config shares it. With `window_concurrency` above 1 the windows share
    /// one view: the counts add up, and the stall check only fires once
    /// every window has gone quiet.
    pub activity: crate::agent_activity::ActivityHandle,
    /// How long an agent call may write nothing before it is stopped as
    /// stalled. `None` turns the check off. Only applies to the claude
    /// provider, the one that streams per token.
    pub idle_timeout: Option<Duration>,
    /// Pre-built provider configs keyed by name, populated at startup.
    provider_configs: std::collections::HashMap<String, LlmProvider>,
}

#[cfg(test)]
impl EnrichConfig {
    /// Build a config for tests (crate-internal), without reading the
    /// environment. Shared by the enrich and document-convert test suites.
    pub(crate) fn for_test(provider: LlmProvider) -> Self {
        let mut provider_configs = std::collections::HashMap::new();
        provider_configs.insert(
            "opencode".to_string(),
            LlmProvider::OpenCode {
                path: "opencode".into(),
                model: None,
            },
        );
        provider_configs.insert(
            "claude".to_string(),
            LlmProvider::Claude {
                path: "claude".into(),
                model: Some("opus".into()),
            },
        );
        EnrichConfig {
            provider,
            timeout: Duration::from_secs(600),
            feedback_timeout: Duration::from_secs(600),
            code_commit: "abc123".to_string(),
            max_rss_mb: 3500,
            // Chunking off by default in tests; chunk tests opt in explicitly.
            max_articles_per_run: 0,
            target_articles: Vec::new(),
            feedback_rounds: FeedbackRounds::default(),
            window_mode: WindowMode::default(),
            window_concurrency: 1,
            steps: RunSteps::all(),
            // Tests that do not opt in run cold, so a fake runner's calls read
            // as they always did; the session tests set this themselves.
            session_reuse: SessionReuse::Off,
            effort: None,
            activity: Default::default(),
            idle_timeout: None,
            provider_configs,
        }
    }
}

impl EnrichConfig {
    /// Build a config for a run outside the worker: the `enrich-once`
    /// binary, which exercises the real loop against a directory on disk.
    /// Deliberately not `from_env`: a local run should say what it does
    /// rather than inherit whatever the shell happens to carry.
    pub fn for_local_run(
        provider: LlmProvider,
        timeout: Duration,
        max_articles: usize,
        target_articles: Vec<String>,
        feedback_rounds: FeedbackRounds,
        effort: Option<String>,
        session_reuse: SessionReuse,
    ) -> Self {
        let mut provider_configs = std::collections::HashMap::new();
        provider_configs.insert("claude".to_string(), provider.clone());
        Self {
            provider,
            timeout,
            feedback_timeout: timeout,
            code_commit: String::new(),
            max_rss_mb: 0,
            max_articles_per_run: max_articles,
            target_articles,
            feedback_rounds,
            window_mode: WindowMode::default(),
            window_concurrency: 1,
            steps: RunSteps::all(),
            session_reuse,
            effort,
            activity: Default::default(),
            // A local run is watched by the person who started it.
            idle_timeout: None,
            provider_configs,
        }
    }

    pub fn from_env() -> Self {
        let provider_name = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "opencode".into());

        let timeout = std::env::var("LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);

        let code_commit = std::env::var("CODE_COMMIT").unwrap_or_default();

        // RSS ceiling for the LLM subprocess. Default 3500 MB leaves headroom
        // under the 4096Mi container limit for the worker, git, and node's
        // baseline plus the ~5s watchdog poll lag.
        let max_rss_mb = std::env::var("ENRICH_MAX_RSS_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3500);

        // Chunk size for large laws: max articles per enrich run. 0 disables
        // chunking entirely (whole-law sessions, the pre-chunking behavior).
        let max_articles_per_run = std::env::var("ENRICH_MAX_ARTICLES_PER_RUN")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(15);

        // Build all provider configs once from env vars
        let opencode_provider = LlmProvider::OpenCode {
            path: std::env::var("OPENCODE_PATH")
                .or_else(|_| std::env::var("LLM_PATH"))
                .unwrap_or_else(|_| "opencode".into())
                .into(),
            model: std::env::var("OPENCODE_MODEL")
                .or_else(|_| std::env::var("LLM_MODEL"))
                .ok(),
        };
        let claude_provider = LlmProvider::Claude {
            path: std::env::var("CLAUDE_PATH")
                .or_else(|_| std::env::var("LLM_PATH"))
                .unwrap_or_else(|_| "claude".into())
                .into(),
            model: std::env::var("CLAUDE_MODEL")
                .or_else(|_| std::env::var("LLM_MODEL"))
                .ok(),
        };

        // Feedback rounds per gate. A bad spec is not worth failing a worker
        // over, but it must not silently read as something else either.
        let feedback_rounds = match std::env::var("ENRICH_FEEDBACK_ROUNDS") {
            Ok(spec) => FeedbackRounds::parse(&spec).unwrap_or_else(|e| {
                tracing::warn!(spec = %spec, error = %e, "ignoring ENRICH_FEEDBACK_ROUNDS");
                FeedbackRounds::default()
            }),
            Err(_) => FeedbackRounds::default(),
        };

        // Session reuse. A bad spec must not silently read as the default —
        // this is the setting whose whole point is being able to say which of
        // the two a run used.
        let session_reuse = match std::env::var("ENRICH_SESSION_REUSE") {
            Ok(spec) => SessionReuse::parse(&spec).unwrap_or_else(|e| {
                tracing::warn!(spec = %spec, error = %e, "ignoring ENRICH_SESSION_REUSE");
                SessionReuse::default()
            }),
            Err(_) => SessionReuse::default(),
        };

        // What a window is. A bad spec must not read as the default silently:
        // this is the setting whose whole point is being able to say which
        // shape a run used.
        let window_mode = match std::env::var("ENRICH_WINDOW_MODE") {
            Ok(spec) => WindowMode::parse(&spec).unwrap_or_else(|e| {
                tracing::warn!(spec = %spec, error = %e, "ignoring ENRICH_WINDOW_MODE");
                WindowMode::default()
            }),
            Err(_) => WindowMode::default(),
        };

        // Windows in flight at once. 1 keeps today's behaviour exactly.
        let window_concurrency = std::env::var("ENRICH_WINDOW_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(1);

        let effort = std::env::var("LLM_EFFORT").ok().filter(|e| !e.is_empty());

        let provider = match provider_name.as_str() {
            "claude" => claude_provider.clone(),
            _ => opencode_provider.clone(),
        };

        let mut provider_configs = std::collections::HashMap::new();
        provider_configs.insert("opencode".to_string(), opencode_provider);
        provider_configs.insert("claude".to_string(), claude_provider);

        Self {
            provider,
            timeout: Duration::from_secs(timeout),
            feedback_timeout: Duration::from_secs(timeout),
            code_commit,
            max_rss_mb,
            max_articles_per_run,
            // A worker walks whole laws; a named entry is a local, targeted
            // instruction and has no env var to arrive through.
            target_articles: Vec::new(),
            feedback_rounds,
            window_mode,
            window_concurrency,
            steps: std::env::var("ENRICH_STEPS")
                .ok()
                .and_then(|spec| match RunSteps::parse(&spec) {
                    Ok(steps) => Some(steps),
                    Err(e) => {
                        tracing::warn!(error = %e, "ENRICH_STEPS unreadable, running every step");
                        None
                    }
                })
                .unwrap_or_else(RunSteps::all),
            session_reuse,
            effort,
            activity: Default::default(),
            idle_timeout: agent_idle_timeout(
                std::env::var("ENRICH_AGENT_IDLE_SECS").ok().as_deref(),
            ),
            provider_configs,
        }
    }

    /// Return a config with the provider overridden if the payload specifies one.
    ///
    /// Selects from pre-built provider configs — no env vars are re-read.
    pub fn with_provider_override(&self, provider_name: &str) -> Self {
        let provider = if let Some(cfg) = self.provider_configs.get(provider_name) {
            cfg.clone()
        } else {
            tracing::warn!(
                requested = %provider_name,
                fallback = %self.provider.name(),
                "unknown provider in payload, falling back to default"
            );
            self.provider.clone()
        };

        Self {
            provider,
            timeout: self.timeout,
            feedback_timeout: self.feedback_timeout,
            code_commit: self.code_commit.clone(),
            max_rss_mb: self.max_rss_mb,
            max_articles_per_run: self.max_articles_per_run,
            target_articles: self.target_articles.clone(),
            feedback_rounds: self.feedback_rounds,
            window_mode: self.window_mode,
            window_concurrency: self.window_concurrency,
            // Carried over like every other field. Hardcoding `all()` here
            // made `ENRICH_STEPS` dead in the worker: the enqueue path always
            // names a provider, so this override runs on every production job
            // and discarded whatever `from_env` had parsed. Running only the
            // closing pass — the use the field's own doc gives as its reason
            // for existing — did nothing.
            steps: self.steps,
            session_reuse: self.session_reuse,
            effort: self.effort.clone(),
            activity: self.activity.clone(),
            idle_timeout: self.idle_timeout,
            provider_configs: self.provider_configs.clone(),
        }
    }
}

/// Build the enrichment branch name for a given provider.
///
/// All enriched laws for a provider live on a single shared branch
/// (`enrich/{provider}`), so results can be compared with main and
/// between providers without branch-per-law proliferation.
pub fn enrich_branch_name(provider_name: &str) -> String {
    format!("enrich/{provider_name}")
}

/// Build the prompt that tells the LLM to follow the skill pipeline.
/// Plan the chain against what this runtime grants, reading each skill's own
/// declaration from disk.
///
/// A skill that cannot be read counts as declaring nothing. The step then runs
/// undegraded and fails later on its artefact if the file really is missing,
/// which is a clearer failure than a capability error about a file that does
/// not exist.
fn chain_plan(
    repo_path: &Path,
) -> (
    Vec<(&'static capabilities::StepSpec, capabilities::StepPlan)>,
    Vec<String>,
) {
    let grant: std::collections::BTreeSet<String> = capabilities::ENRICH_GRANT
        .iter()
        .map(|t| (*t).to_owned())
        .collect();
    let read: Vec<(&'static capabilities::StepSpec, Option<String>)> = capabilities::CHAIN
        .iter()
        .map(|spec| {
            (
                spec,
                std::fs::read_to_string(repo_path.join(spec.skill)).ok(),
            )
        })
        .collect();
    let plan = read
        .iter()
        .map(|(spec, markdown)| {
            (
                *spec,
                capabilities::plan_step(spec, &grant, markdown.as_deref()),
            )
        })
        .collect();
    (plan, capabilities::ungranted(&grant, &read))
}

/// The instruction body of one step, without its heading.
///
/// Kept here rather than in [`capabilities`] because this is prompt wording,
/// and the capability module decides which steps exist rather than what they
/// say. `chunked` switches the generate and reverse-validation steps to the
/// article subset; everything else is identical between the two shapes, which
/// is why they are no longer two prompts.
fn step_body(spec: &capabilities::StepSpec, chunked: bool) -> String {
    let scope_generate = if chunked {
        " — restricted to the article subset listed above"
    } else {
        ""
    };
    let scope_reverse = if chunked {
        " — only for the articles you edited in this session"
    } else {
        ""
    };
    match spec.name {
        "MvT research" => format!(
            "Read {} and follow its instructions to search for Memorie van Toelichting\n\
             documents and generate Gherkin test scenarios.",
            spec.skill
        ),
        "Generate machine_readable" => format!(
            "Read {} and its reference.md and examples.md.\n\
             Create machine_readable sections for each executable article{scope_generate}.",
            spec.skill
        ),
        "Reverse validation" => format!(
            "Read {} and follow its instructions to verify every element in\n\
             machine_readable traces back to the original legal text{scope_reverse}.",
            spec.skill
        ),
        other => format!("Read {} and follow its instructions. ({other})", spec.skill),
    }
}

/// Build the translation prompt from the planned chain.
///
/// Steps that the runtime cannot support are absent rather than present and
/// impossible. A degraded step carries the note that names what it cannot do
/// and what runs in its place, because an agent that is only told a tool is
/// missing still has to decide what to do about the instruction that needed
/// it, and in round 2 it decided to report success.
///
/// `skip_mvt` is honoured on top of the plan: an earlier session in the same
/// law may already have produced the feature file.
fn build_prompt(
    yaml_path: &str,
    progress_file_path: &str,
    plan: &[(&'static capabilities::StepSpec, capabilities::StepPlan)],
    chunk_articles: Option<&[String]>,
    _skip_mvt: bool,
    has_brief: bool,
) -> String {
    let mut out =
        String::from("You are interpreting a Dutch law to make it machine-executable.\n\n");
    let _ = writeln!(out, "The law YAML file is: {yaml_path}\n");

    if let Some(articles) = chunk_articles {
        let _ = writeln!(
            out,
            "This is one chunk of a larger law. Process ONLY these articles (by their\n\
             `number` field) and leave every other article completely untouched:\n\n{}\n",
            articles.join(", ")
        );
    }

    if has_brief {
        let _ = writeln!(
            out,
            "Read `{}` in the same directory first. It states, per article in scope,\n\
             where the article sits in the document structure, which definition\n\
             provisions govern it, and which other articles of this law modify it.\n\
             An article that another article bends must be translated as they leave\n\
             it, not as it reads alone.\n",
            context::CONTEXT_BRIEF
        );
    }

    out.push_str(
        "Follow this pipeline in order. Every step it does not list is a step this\n\
         runtime cannot support; do not perform it from memory and do not report it.\n\n",
    );

    let mut number = 0usize;
    let mut omitted: Vec<String> = Vec::new();
    for (spec, step_plan) in plan {
        if !step_plan.is_in_prompt() {
            omitted.push((*spec.name).to_string());
            continue;
        }
        number += 1;
        let _ = writeln!(out, "## Step {number}: {}", spec.name);
        out.push_str(&step_body(spec, chunk_articles.is_some()));
        out.push('\n');
        if let capabilities::StepPlan::Degraded { missing } = step_plan {
            let _ = writeln!(out, "\n{}", capabilities::degraded_note(missing));
        }
        out.push('\n');
    }

    if !omitted.is_empty() {
        let _ = writeln!(
            out,
            "Not part of this run: {}. Produce nothing that would have come from them.\n",
            omitted.join(", ")
        );
    }

    // Unconditional, and it used to hang off the sentence above. When the
    // retrieval step left the chain nothing was omitted any more, so the rule
    // left with it. The pull towards a remembered citation does not need a
    // missing step to invite it: round 2 answered with a kst- number for a
    // document it never opened, and to whoever reads the law afterwards that
    // reads exactly like a citation someone checked.
    out.push_str(
        "Cite no source you have not read in this session. If you believe a source exists, \
         name it as a lead in prose without a number and without a link: a lead invites \
         someone to look, a citation claims someone already did.\n\n",
    );

    number += 1;
    let _ = writeln!(
        out,
        "## Step {number}: Session report\n\
         Write (or update) the file `.enrichment-result.yaml` next to the law YAML with\n\
         a `chunk_report` mapping recording what you did in this session:\n\
         \n\
         ```yaml\n\
         chunk_report:\n\
         \x20 articles_reviewed: [\"<number>\", ...]\n\
         \x20 articles_skipped:\n\
         \x20   - number: \"<number>\"\n\
         \x20     reason: \"<why no machine_readable, e.g. definition/transitional article>\"\n\
         ```\n\
         \n\
         This report is REQUIRED even when no article needed a machine_readable\n\
         section. Keep any existing `related_legislation` entries in that file intact.\n"
    );

    let _ = writeln!(
        out,
        "Write all changes to disk. Do not ask questions — proceed autonomously.\n\
         \n\
         ## Progress tracking\n\
         Before starting each step, write a JSON progress file to: {progress_file_path}\n\
         Use the Write tool, one brief write per phase transition, with the fields\n\
         `phase`, `step` and `total_steps`."
    );

    out
}

/// The names a law file may use to record what its model does not do.
///
/// Schema v0.7.0 folded four fields into two and set
/// `additionalProperties: false`, so the old names are now rejected by the
/// schema gate and the new names are rejected by every schema before it. The
/// feedback prompt therefore cannot name one set of fields for every file: a
/// prompt that prescribes what the file's own schema forbids turns the repair
/// round into a loop that hands the agent the same impossible instruction
/// every time. Which set applies is a property of the file, so it is read off
/// the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vocabulary {
    /// Schema v0.7.0 and later: `markings` and `open_terms`.
    Markings,
    /// Schema v0.5.6 and earlier: `untranslatables` and `norm_gaps`. Laws on
    /// these versions stay in the corpus and are re-enriched, so the wording
    /// they get is kept exactly as it was.
    Legacy,
}

/// The vocabulary a law file's declared schema version allows.
///
/// Unreadable, unparseable or an unregistered `$schema` all fall back to
/// [`Vocabulary::Markings`]: that is what new work targets, and a file the
/// schema gate cannot place is a file the gate is about to fail on anyway.
async fn vocabulary_of(yaml_abs: &Path) -> Vocabulary {
    match tokio::fs::read_to_string(yaml_abs).await {
        Ok(raw) => vocabulary_of_yaml(&raw),
        Err(e) => {
            tracing::warn!(
                path = %yaml_abs.display(),
                error = %e,
                "cannot read law for its schema version; assuming the current vocabulary"
            );
            Vocabulary::Markings
        }
    }
}

/// [`vocabulary_of`] on already-read YAML text.
fn vocabulary_of_yaml(raw: &str) -> Vocabulary {
    let Ok(value) = serde_yaml_ng::from_str::<serde_json::Value>(raw) else {
        return Vocabulary::Markings;
    };
    match regelrecht_engine::schema::detect_version(&value) {
        Some(version) if !schema_has_markings(version) => Vocabulary::Legacy,
        _ => Vocabulary::Markings,
    }
}

/// Whether a `vMAJOR.MINOR.PATCH` schema version is v0.7.0 or later, the point
/// at which `markings` replaced `untranslatables` and `norm_gaps`.
///
/// The boundary is v0.7.0 exactly. v0.6.0 still defines `untranslatables` and
/// has no `markings`, and `machine_readable` is `additionalProperties: false`,
/// so handing an agent the new vocabulary for a v0.6.x law yields a file that
/// its own schema rejects.
fn schema_has_markings(version: &str) -> bool {
    match regelrecht_shared::parse_schema_version(version) {
        Some(parsed) => parsed >= (0, 7, 0),
        // An unparseable version is not an old one: treat it as current
        // rather than sending an agent back to fields the schema dropped.
        _ => true,
    }
}

/// Prompt for a feedback pass: the artefact exists and a gate found
/// something.
///
/// Two things make the difference between a useful round and a harmful one.
/// The findings go over verbatim, because the agent cannot run the check
/// itself and a paraphrase loses the instance path that says where the
/// problem sits. And what counts as an answer depends on the gate: a schema
/// error must be fixed, while a question the text raises may equally be
/// answered by recording why the model does not do what the check expected.
///
/// `vocabulary` decides which field names the prompt may prescribe; see
/// [`Vocabulary`].
fn build_feedback_prompt(yaml_path: &str, feedback: &Feedback, vocabulary: Vocabulary) -> String {
    let list = feedback
        .findings
        .iter()
        .map(|f| format!("- {f}"))
        .collect::<Vec<_>>()
        .join("\n");

    // The citation half of the marking gate is about evidence, not about
    // fields, so it reads the same under either vocabulary.
    const CITATION_RULE: &str = "**Do not cite what you were not given.** You have no network and \
         no search. A kamerstuk, a Staatsblad number or a URL that appears in none of the text in \
         front of you is something you recall, and a recalled citation is indistinguishable from \
         a read one to whoever comes after. If you believe a source exists, name it as a lead in \
         prose (\"the explanatory memorandum probably covers this\") without a number and without \
         a link. A lead invites someone to look; a citation claims someone already did.";

    let (what_happened, what_to_do) = match (feedback.gate, vocabulary) {
        // Deliberately the narrowest prompt in the chain, and mostly a list of
        // what not to do. The law has been through every gate already; this
        // pass exists only because the windows walked it in pieces and an
        // early window could not bind to a name a later one had yet to invent.
        (Gate::Reconcile, _) => (
            "is finished, and one last question is left: a value it reads by hand may be a value \
             it now produces somewhere else",
            "Every entry of this law has been translated and every gate has run. You are not \
             here to translate anything.\n\n\
             The findings below name places where this law reads a value as a bare input, or \
             leaves it as an open term, while another entry of this same law now declares an \
             output for the same thing. That happens because the law was walked in windows: the \
             entry that reads the value was written while the entry that produces it had no \
             model yet, so the binding could not be made there.\n\n\
             **Connect what already exists, and nothing else.** Where the finding is right, turn \
             the input into one with a `source` that names the output and passes the parameters \
             that entry asks for. Where it is wrong — the two names look alike but mean different \
             things, or the law really does leave the value to somebody else — say so in one \
             sentence and leave the file as it is.\n\n\
             **What you may not do**, and each of these makes this pass a deterioration:\n\
             - remodel an entry, change an `operation`, an `action` or a `condition`;\n\
             - add a `marking`, an `open_term` or a `norm_gap`, or remove one on grounds of \
             content;\n\
             - rename an existing output or input so that two names match;\n\
             - touch an entry no finding names."
                .to_string(),
        ),
        // Every window has been walked and the closing pass has connected
        // what it could, so what is left here resolves to nothing at all.
        (Gate::Binding, _) => (
            "is finished, and still reads values that nothing in this law provides",
            "Every entry of this law has been translated and the closing pass has run. Each \
             finding below is a reference the engine will fail on the moment the law is \
             executed: a `$name` that no parameter, input, output, definition or open term of \
             this law declares, or a `source` without `regulation` that names an output no \
             entry of this law produces.\n\n\
             **Make each reference resolve.** Usually the name is a slip: correct it to the \
             name this law does declare. Where the value really is missing, declare it in the \
             entry the finding names: a `parameter` when the caller supplies it, an `input` \
             with a `source` when another law or an external register supplies it. A `source` \
             to an output of this law must name an output that exists; when the value comes \
             from another law, add the `regulation`.\n\n\
             **A marking or an open term is not an answer here.** Neither makes a name exist, \
             and the law stays unable to run.\n\n\
             Touch only the entries the findings name, and change nothing else in them."
                .to_string(),
        ),
        (Gate::Schema, _) => (
            "does not validate against the regelrecht JSON schema",
            "Fix it in place so it validates. Change as little as possible.".to_string(),
        ),
        (Gate::Marking, Vocabulary::Markings) => (
            "records things in the wrong drawer, or names a source it cannot have read",
            format!(
                "Two things, and they are about the record rather than about the translation.\n\n\
                 **Put each marking in the drawer it belongs to.** Which drawer follows from who \
                 resolves it, and there are three.\n\n\
                 A value that another law produces is not a gap at all: it is an input with a \
                 `source` naming that regulation and the output it asks for.\n\n\
                 A norm whose content is filled in elsewhere — by a lower regulation, or, when \
                 the law appoints nobody (\"redelijkerwijs\", \"in bijzondere gevallen\"), by the \
                 executive policy of whichever authority applies it — is an `open_term`. The \
                 format expresses it fine; the content lives somewhere else. Whether that \
                 somewhere else is already in the corpus is a state of the corpus and does not \
                 belong in the law file.\n\n\
                 Only what the format itself cannot express is a `marking`: `resolution: operation` \
                 when the operation does not exist and has to be built, `resolution: model` when \
                 the operation set is not the problem and the format has no shape for the \
                 construct. Recording an open term as a marking sends it to a queue where nobody \
                 will ever work it.\n\n\
                 **A marking has three prose fields and they do different work.** `about` names \
                 the construct in the words the article uses. `reason` says why it does not fit, \
                 in terms of what the format does have: name the shape or the operation that \
                 comes closest and say where it falls short. `resolved_by` names the change that \
                 would close the gap, concretely enough to become work. The order is not free: \
                 the change follows from the reading, and the reading cannot be recovered from \
                 the change. \"Voor zover\" beperkt de toepassing per bepaling van deze wet en \
                 niet de wet als geheel, en het model kent alleen toepasselijkheid van een hele \
                 wet — that is a reason, and the form the format would need follows from it. \
                 \"Dit past niet in het model\" is not a reason, it is the marking repeating \
                 itself. A reason that restates `about`, or that is the wanted change said \
                 twice, leaves a reader unable to tell a gap somebody examined from a gap \
                 nobody opened.\n\n\
                 **A flag is a flag on an article that is otherwise worked out.** It names the \
                 one thing that does not fit and leaves everything that does fit standing; an \
                 article whose whole model is a marking, or a single open term, is a defect. \
                 That holds for both drawers: moving a gap from one to the other does not make \
                 the article any less empty. `target` names the values in this article that \
                 cannot be produced because of it, and it may only name values this model \
                 declares itself, so an invented name is not an escape. An empty `target` is a \
                 statement rather than an omission: it says the article stays executable. \
                 `legal_text_excerpt` quotes this article's own words, because a marking that \
                 cannot quote what it is about is about something else.\n\n\
                 **An open term names who fills it.** `delegated_to` when the article appoints \
                 an authority, `expected_source` when it names the regulation, \
                 `decided_per_case_by` when it appoints nobody and the competent authority \
                 decides case by case with a motivation. An open term that names none of the \
                 three is not an open norm but an omission, and it reads as though the law left \
                 something open that it did not.\n\n\
                 {CITATION_RULE}"
            ),
        ),
        (Gate::Marking, Vocabulary::Legacy) => (
            "records things in the wrong drawer, or names a source it cannot have read",
            format!(
                "Two things, and they are about the record rather than about the translation.\n\n\
                 **Put each marking in the drawer it belongs to.** An `untranslatable` says the \
                 engine's operations cannot express a construct, and it is resolved by building \
                 the operation. A `norm_gap` says the norm is open and gets its content from a \
                 regulation or beleidsregel that is not in the corpus, and it is resolved by \
                 harvesting that source or, absent one, by the competent authority deciding per \
                 case. Recording the second as the first sends it to a queue where nobody will \
                 ever work it.\n\n\
                 {CITATION_RULE}"
            ),
        ),
        (Gate::Checks, Vocabulary::Markings) => (
            "raised questions that the statutory text puts to the model",
            "Answer every one of them. There are two acceptable answers and you \
             choose per finding.\n\n\
             **Change the model**, when the text does say what the check \
             expected and the model missed it. A lid that derogates from \
             another needs a branch; an article that computes over a \
             berekeningsjaar needs a binding that carries that period.\n\n\
             **Record why not**, when the text does not support what the check \
             expected. Use an `open_term` when the norm is open and its content \
             is filled in by a lower regulation or by the executive policy of \
             the authority that applies it, and a `marking` when the format \
             itself cannot express the construct — `resolution: operation` for an \
             operation that has to be built, `resolution: model` for a shape \
             the format does not have. A value that another law produces is \
             neither: that is an input with a `source`. Both are first-class \
             answers and neither is an admission of failure.\n\n\
             **Both answers go in the law file, in the article the finding \
             names.** The check runs over the law YAML and reads nothing else, \
             so a note in `.enrichment-result.yaml` or any other sidecar is not \
             an answer to it: the finding comes back next round word for word, \
             and the reasoning you wrote is read by nobody. If your answer is \
             that the check expected the wrong thing, that reasoning belongs in \
             the `reason` of a marking or in the description of an open term, \
             where it sits beside the thing it is about.\n\n\
             A marking flags the one thing that does not fit and leaves the \
             rest of the article standing. What you may not do is leave a \
             finding unanswered, or make it go away by removing the logic it \
             was about."
                .to_string(),
        ),
        (Gate::Checks, Vocabulary::Legacy) => (
            "raised questions that the statutory text puts to the model",
            "Answer every one of them. There are two acceptable answers and you \
             choose per finding.\n\n\
             **Change the model**, when the text does say what the check \
             expected and the model missed it. A lid that derogates from \
             another needs a branch; an article that computes over a \
             berekeningsjaar needs a binding that carries that period.\n\n\
             **Record why not**, when the text does not support what the check \
             expected. Use `untranslatables` when the engine's operations \
             cannot express the construct, and `norm_gaps` when the norm is \
             open and gets its content from a regulation or beleidsregel that \
             is not in the corpus. Both are first-class answers and neither is \
             an admission of failure.\n\n\
             What you may not do is leave a finding unanswered, or make it go \
             away by removing the logic it was about."
                .to_string(),
        ),
    };

    let marking_note = match (feedback.gate.accepts_marking(), vocabulary) {
        (true, _) => "",
        (false, Vocabulary::Markings) => {
            "\n\nDo not delete a `machine_readable` section to make an error go \
             away. A construct the format cannot express belongs in `markings`, \
             with the words it hangs on and the change that would resolve it; a \
             norm whose content is filled in elsewhere belongs in `open_terms`."
        }
        (false, Vocabulary::Legacy) => {
            "\n\nDo not delete a `machine_readable` section to make an error go \
             away. A construct that cannot be expressed belongs in \
             `untranslatables` with a reason."
        }
    };

    format!(
        r#"The law YAML you just wrote {what_happened}.

The file is: {yaml_path}

{what_to_do}

**Do not touch any `text` field.** Those hold the statutory text and are not
yours to edit.{marking_note}

The findings, verbatim:
{list}

Write the result back to the same file. Do not ask questions."#
    )
}

/// Build the prompt for one enrichment chunk: an explicit article subset.
///
/// Differences from `build_prompt` (which stays byte-identical for whole-law
/// runs): the agent must process ONLY the listed articles; the MvT-research
/// step is skipped on continuation chunks (`skip_mvt`, cursor > 0 — the
/// feature file already exists on the branch); reverse validation is limited
/// to the articles edited this session; and the agent must record a
/// `chunk_report` in `.enrichment-result.yaml` so a legitimately-empty chunk
/// (e.g. transitional law) still proves it was reviewed.
/// Compute the path of the progress file for a given law YAML file.
///
/// The progress file sits next to the YAML (e.g.
/// `regulation/nl/wet/foo/.enrichment-progress.json`).
pub fn progress_file_path(yaml_abs: &Path) -> PathBuf {
    yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment-progress.json")
}

/// Allowlisted environment variable prefixes/names that are safe to pass to the
/// LLM subprocess.  Everything else (DATABASE_URL, etc.) is stripped.
const LLM_ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "TERM",
    "LANG",
    "USER",
    "SHELL",
    "TMPDIR",
    "XDG_",
    // Provider-specific auth.
    //
    // NOTE: ANTHROPIC_API_KEY is deliberately NOT forwarded. The claude provider
    // authenticates only with a personal subscription via CLAUDE_CODE_OAUTH_TOKEN.
    // Keeping ANTHROPIC_API_KEY out of the subprocess env means that even if it is
    // still set on the worker (e.g. a leftover), it can never reach `claude` and
    // silently take precedence over the OAuth token — the exact footgun that
    // makes claude fail auth at startup.
    "CLAUDE_CODE_OAUTH_TOKEN",
    "VLAM_API_KEY",
    "OPENCODE_",
];

/// Check whether an environment variable name is on the allowlist.
fn env_allowed(key: &str) -> bool {
    LLM_ENV_ALLOWLIST
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// Select one Claude OAuth token from a comma-separated list, rotating by a
/// time `bucket` so consecutive runs spread across several personal
/// subscriptions (each token has its own usage/rate limits).
///
/// `CLAUDE_CODE_OAUTH_TOKEN` may hold multiple tokens separated by commas. The
/// chosen index is `bucket % n`; callers pass `unix_secs / 100`, so the active
/// token rotates roughly every 100 seconds. Returns `(index, count, token)`, or
/// `None` when there are no non-empty tokens. Pure so it can be unit-tested.
fn select_claude_token(raw: &str, bucket: u64) -> Option<(usize, usize, &str)> {
    let tokens: Vec<&str> = raw
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let idx = (bucket % tokens.len() as u64) as usize;
    Some((idx, tokens.len(), tokens[idx]))
}

/// Build the command for the configured LLM provider.
///
/// The subprocess gets a stripped environment: only variables on
/// `LLM_ENV_ALLOWLIST` are forwarded.  This prevents leaking DATABASE_URL
/// and other secrets to the LLM process (which may have shell access).
///
/// `file_arg` is passed to OpenCode as its `-f` input file (the Claude provider
/// ignores it and reads via its own tools from `cwd`). Enrich passes the law
/// YAML; a caller with no single input file passes `None`.
fn build_command(
    provider: &LlmProvider,
    prompt: &str,
    file_arg: Option<&Path>,
    cwd: &Path,
    tools: ToolPolicy<'_>,
    effort: Option<&str>,
    session: SessionAction,
) -> tokio::process::Command {
    // Collect allowed env vars before creating the command.
    let safe_env: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| env_allowed(k)).collect();

    // Diagnostic logging: record exactly what will be spawned and which env vars
    // are forwarded (NAMES only — never values). Classify the OAuth token by its
    // non-secret prefix so a misconfiguration (an API key pasted into the OAuth
    // slot) is obvious, and flag whether ANTHROPIC_API_KEY is still present in the
    // worker env even though it is deliberately never forwarded.
    let forwarded_env: Vec<&str> = safe_env.iter().map(|(k, _)| k.as_str()).collect();
    let oauth_token_kind = std::env::var("CLAUDE_CODE_OAUTH_TOKEN").ok().map(|t| {
        if t.is_empty() {
            "empty"
        } else if t.starts_with("sk-ant-oat") {
            "oauth-token (sk-ant-oat…)"
        } else if t.starts_with("sk-ant-api") {
            "WRONG: looks like an API key (sk-ant-api…)"
        } else {
            "unrecognized-prefix"
        }
    });
    let model = match provider {
        LlmProvider::OpenCode { model, .. } | LlmProvider::Claude { model, .. } => model.as_deref(),
    };
    tracing::info!(
        provider = provider.name(),
        model = ?model,
        effort = ?effort,
        prompt_chars = prompt.len(),
        claude_oauth_token_kind = ?oauth_token_kind,
        anthropic_api_key_present_in_worker_env = std::env::var_os("ANTHROPIC_API_KEY").is_some(),
        "spawning LLM subprocess"
    );
    // The forwarded env var NAMES are static between spawns — keep them at debug
    // so they don't add a long line to every job's info logs.
    tracing::debug!(provider = provider.name(), forwarded_env = ?forwarded_env, "forwarded env to LLM subprocess");

    let mut cmd = match provider {
        LlmProvider::OpenCode { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
            cmd.arg("run").arg(prompt);
            if let Some(f) = file_arg {
                cmd.arg("-f").arg(f);
            }
            cmd.arg("--format").arg("json").arg("--dir").arg(cwd);
            if let Some(ref m) = model {
                cmd.arg("-m").arg(m);
            }
            // OpenCode has no effort flag; saying so beats passing an option
            // it would reject and letting the run fail on a typo in config.
            if effort.is_some() {
                tracing::warn!("effort is not supported by opencode; ignored");
            }
            // Session reuse is a claude-provider affair. Under opencode every
            // call stays cold, and it says so rather than letting a run be
            // read as though the windows had been shared.
            if !matches!(session, SessionAction::Cold) {
                tracing::warn!("session reuse is not wired for opencode; this call runs cold");
            }
            cmd
        }
        LlmProvider::Claude { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
            // If CLAUDE_CODE_OAUTH_TOKEN holds several comma-separated tokens,
            // override the forwarded value with a single one chosen by a
            // time-rotating index, so load spreads across the subscriptions.
            if let Ok(raw) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
                let bucket = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() / 100)
                    .unwrap_or(0);
                if let Some((idx, count, token)) = select_claude_token(&raw, bucket) {
                    // Always apply the selected (trimmed) token — even for a
                    // single token — so stray whitespace or a trailing comma
                    // never reaches claude verbatim. Never log the token value;
                    // only the 1-based position and count.
                    cmd.env("CLAUDE_CODE_OAUTH_TOKEN", token);
                    if count > 1 {
                        tracing::info!(
                            using_token = idx + 1,
                            of_tokens = count,
                            "selected claude oauth token (rotating by ~100s)"
                        );
                    }
                }
            }
            // Leaving a tool off `--allowedTools` does not take it away: that
            // flag auto-approves, and what it omits merely has to be asked for,
            // which the checkout's own settings then grant. Withholding needs
            // `--disallowedTools`. Without this the enrichment agent made
            // twenty `Bash` calls in a session whose plan reported the step as
            // running degraded for want of a shell.
            let denied = tools.denied();
            cmd.arg("-p")
                .arg(prompt)
                .arg("--allowedTools")
                .arg(tools.allowed())
                // One JSON event per line while the run goes, closing with
                // the `result` event that carries the token use. The worker
                // reads the events for progress and to notice a stalled
                // run; `--include-partial-messages` adds an event per token
                // so a long answer does not look like silence. stream-json
                // requires `--verbose` under `-p`.
                .arg("--output-format")
                .arg("stream-json")
                .arg("--verbose")
                .arg("--include-partial-messages")
                .current_dir(cwd);
            if !denied.is_empty() {
                cmd.arg("--disallowedTools").arg(denied.join(","));
            }
            if let Some(ref m) = model {
                cmd.arg("--model").arg(m);
            }
            // Effort rides beside the model because that is how the CLI takes
            // it: one flag for which model, one for how hard it thinks.
            if let Some(e) = effort {
                cmd.arg("--effort").arg(e);
            }
            // The window's session. The worker chooses the id rather than
            // reading it back out of the provider's closing JSON: an id we
            // hand in is known before the process starts, so a run that dies
            // before it prints anything still leaves a resumable session, and
            // nothing has to be parsed to continue.
            match session {
                SessionAction::Cold => {}
                SessionAction::Start(id) => {
                    cmd.arg("--session-id").arg(id.to_string());
                }
                SessionAction::Resume(id) => {
                    cmd.arg("--resume").arg(id.to_string());
                }
            }
            cmd
        }
    };

    // Run the agent as its own process-group leader so a timeout/memory kill can
    // signal the whole tree (the CLI plus any node workers or git it forks), not
    // just the direct child. `kill_on_drop` is a backstop: if the worker future
    // is dropped (panic, early return) the child is reaped rather than orphaned.
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    cmd
}

/// Result of preparing the per-job enrichment checkout: the client plus the
/// base-branch blob SHA of the target law (recorded into `.enrichment.yaml`
/// as `source_hash`).
pub struct EnrichCorpus {
    pub client: CorpusClient,
    pub source_hash: String,
}

/// Read the `source_hash` recorded in the target law's `.enrichment.yaml`, if
/// present and non-empty. Returns `None` when the file is absent/unparseable
/// or the field is empty (both treated as "unknown provenance").
async fn read_stored_source_hash(repo_path: &Path, normalized_law_path: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Provenance {
        #[serde(default)]
        source_hash: String,
    }
    let meta_rel = Path::new(normalized_law_path)
        .parent()?
        .join(".enrichment.yaml");
    let content = tokio::fs::read_to_string(repo_path.join(meta_rel))
        .await
        .ok()?;
    let prov: Provenance = serde_yaml_ng::from_str(&content).ok()?;
    (!prov.source_hash.is_empty()).then_some(prov.source_hash)
}

/// Read the chunked-enrichment cursor recorded in the target law's
/// `.enrichment.yaml`: `(enrich_cursor, enrich_cursor_path)`.
///
/// Absent file, unparseable YAML, or missing fields all degrade to `(0, "")` —
/// [`plan_chunk`] then resets to the start, which is the safe default for
/// legacy metadata written before the cursor existed.
async fn read_stored_cursor(
    repo_path: &Path,
    normalized_law_path: &str,
) -> (usize, String, String) {
    #[derive(serde::Deserialize, Default)]
    struct CursorFields {
        #[serde(default)]
        enrich_cursor: usize,
        #[serde(default)]
        enrich_cursor_path: String,
        #[serde(default)]
        enrich_cursor_mode: String,
    }
    let Some(parent) = Path::new(normalized_law_path).parent() else {
        return (0, String::new(), String::new());
    };
    let meta_rel = parent.join(".enrichment.yaml");
    let Ok(content) = tokio::fs::read_to_string(repo_path.join(meta_rel)).await else {
        return (0, String::new(), String::new());
    };
    let fields: CursorFields = serde_yaml_ng::from_str(&content).unwrap_or_default();
    (
        fields.enrich_cursor,
        fields.enrich_cursor_path,
        fields.enrich_cursor_mode,
    )
}

/// Create a `CorpusClient` for the enrichment branch.
///
/// Clones the base corpus config but sets the branch to the enrichment branch.
/// The client's `ensure_repo()` will auto-create the branch if it doesn't exist.
///
/// Each invocation uses a unique checkout directory (keyed by branch + job ID)
/// to prevent concurrent workers from clobbering each other's checkouts.
///
/// Uses sparse checkout to only materialize the law directory being enriched
/// plus the `features/` directory. This prevents the LLM subprocess from
/// indexing the entire corpus (thousands of files), which would exceed context
/// limits and cause excessive memory usage.
pub async fn create_enrich_corpus(
    base_config: &CorpusConfig,
    branch: &str,
    job_id: Uuid,
    yaml_path: &str,
) -> Result<EnrichCorpus> {
    let mut config = base_config.clone();
    config.branch = branch.into();

    // Normalize the yaml_path to strip legacy absolute prefixes (e.g.
    // `/tmp/corpus-repo/regulation/…`) before deriving the law directory
    // for sparse checkout. Without this, git sparse-checkout would receive
    // an absolute path it cannot handle.
    let normalized = normalize_yaml_path(yaml_path)?;

    let law_dir = Path::new(&normalized)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|d| !d.is_empty());

    // Sparse checkout: only the law directory + features/
    if let Some(ref dir) = law_dir {
        config.sparse_paths = Some(vec![dir.clone(), "features".to_string()]);
    }

    // Use a separate checkout directory per branch + job to avoid conflicts
    // between concurrent workers processing different laws on the same branch.
    let dir_name = format!("{}-{}", branch.replace('/', "-"), job_id);
    let base_dir = config
        .repo_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("/tmp"));
    config.repo_path = base_dir.join(dir_name);

    let mut client = CorpusClient::new(config);
    client.ensure_repo().await?;

    // Past `ensure_repo()` a per-job git clone exists on disk. Resolving the
    // base branch, checking freshness, or checking out the law can all fail —
    // including a deliberate `BaseDrift` bail-out. The worker's shared cleanup
    // only captures the checkout path on the success path, so on any error we
    // must remove the clone here; otherwise an errored (and especially a
    // drifted, awaiting-human) job leaks its clone on a disk/OOM-sensitive
    // worker.
    let checkout_dir = client.repo_path().to_path_buf();
    let source_hash = match resolve_enrich_base(&client, base_config, &normalized).await {
        Ok(source_hash) => source_hash,
        Err(e) => {
            if let Err(rm) = tokio::fs::remove_dir_all(&checkout_dir).await {
                tracing::warn!(
                    path = %checkout_dir.display(),
                    error = %rm,
                    "failed to clean up per-job corpus checkout after enrich setup error"
                );
            }
            return Err(e);
        }
    };

    // The law is on disk now, so the laws it cites can be named and added to
    // the sparse checkout. Without this the context brief walks a directory
    // holding one law, finds none of the citations, and tells the agent they
    // are absent from the corpus — which is false, and which the agent acts
    // on by marking a binding it could have made. RFC-026 requirement 6 is
    // otherwise inoperative in the only environment that runs it.
    if let Err(e) = widen_to_cited_laws(&client, &normalized).await {
        tracing::warn!(error = %e, "could not widen the checkout to the cited laws; the brief will report them as absent");
    }

    Ok(EnrichCorpus {
        client,
        source_hash,
    })
}

/// Widen the sparse checkout to the laws this one cites.
///
/// Reads the `references` blocks the harvester writes and hands the BWB
/// numbers to the corpus client, which resolves them against the commit.
/// Best effort by design: a citation that resolves to nothing is exactly the
/// case the brief already reports honestly as "not in this corpus".
async fn widen_to_cited_laws(client: &CorpusClient, normalized_path: &str) -> Result<()> {
    let body = tokio::fs::read_to_string(client.repo_path().join(normalized_path)).await?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&body)
        .map_err(|e| PipelineError::Enrich(format!("cannot parse law for its citations: {e}")))?;
    let own_bwb = doc
        .get("bwb_id")
        .and_then(serde_yaml_ng::Value::as_str)
        .unwrap_or_default();
    let numbers: Vec<String> = doc
        .get("articles")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .map(|articles| {
            articles
                .iter()
                .filter_map(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let cited: Vec<String> =
        crate::enrich_v2::context::window_citations(&doc, &numbers, &numbers, own_bwb)
            .into_iter()
            .map(|(bwb, _, _)| bwb)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    client.widen_to_laws(&cited).await?;
    Ok(())
}

/// Resolve the enrichment base branch and materialize the target law from it,
/// returning the base blob SHA to record as provenance (`source_hash`).
///
/// Split out from [`create_enrich_corpus`] so that every error it can raise — a
/// git probe failure, a checkout failure, or a [`PipelineError::BaseDrift`]
/// bail-out — flows through a single caller-side cleanup that removes the
/// per-job clone. Only `&self` methods are used; the clone already exists.
async fn resolve_enrich_base(
    client: &CorpusClient,
    base_config: &CorpusConfig,
    normalized: &str,
) -> Result<String> {
    // Prefer the worker's own base branch (e.g. `pr574`) so PR deployments
    // enrich their own harvested YAML, not production's. Probe the remote
    // first and fall back to `development` only when the branch doesn't
    // exist yet — which covers a fresh PR whose harvester hasn't pushed.
    // Probing explicitly (instead of try-then-fallback on any error)
    // prevents an unrelated `checkout` or `reset` failure from silently
    // dropping the enrichment back to production's branch.
    //
    // The freshness guard below works on the exact file path (not the
    // directory): `is_tracked` and `fetch_base_blob_sha` resolve a single blob,
    // so a newly harvested version of an already-known law is judged on its own
    // path rather than being masked by a sibling version in the same directory.
    let preferred_base = base_config.branch.as_str();
    let preferred_exists = if preferred_base == "development" || branch_is_known(preferred_base) {
        true
    } else {
        let exists = client.remote_branch_exists(preferred_base).await?;
        if exists {
            remember_branch(preferred_base);
        }
        exists
    };
    let base_branch = pick_enrich_base(preferred_base, preferred_exists);
    if !preferred_exists {
        tracing::info!(
            branch = %preferred_base,
            "base branch not yet published on remote, using development for first enrichment"
        );
    }

    // Freshness guard: compare the target law's base version against the
    // provenance recorded in a prior enrichment. New law -> check out fresh;
    // unchanged base -> keep existing enrichment; missing provenance (a legacy,
    // pre-guard enrichment) -> adopt the current base as baseline and proceed;
    // changed base -> fail loudly (do NOT auto-overwrite on a moved base).
    let base_sha = client.fetch_base_blob_sha(base_branch, normalized).await?;
    let tracked = client.is_tracked(normalized).await?;
    let stored = read_stored_source_hash(client.repo_path(), normalized).await;

    match decide_base_action(tracked, stored.as_deref(), &base_sha) {
        BaseAction::CheckoutFresh => {
            client.checkout_path_from_fetch_head(normalized).await?;
            tracing::info!(base = %base_branch, path = %normalized, "checked out law fresh from base");
        }
        BaseAction::Skip => {
            tracing::debug!(path = %normalized, "base unchanged, no fresh checkout needed");
        }
        BaseAction::AdoptBaseline => {
            // Legacy enrichment with no recorded provenance. Keep the existing
            // enrichment (no fresh checkout, like Skip); the current base blob
            // SHA returned below is stamped as `source_hash` on the next
            // `.enrichment.yaml` write, establishing the baseline so subsequent
            // runs can detect real drift.
            tracing::info!(
                path = %normalized,
                base = %base_branch,
                "no recorded provenance for tracked law; adopting current base as baseline (legacy enrichment grandfathered)"
            );
        }
        BaseAction::Drift => {
            return Err(PipelineError::BaseDrift {
                yaml_path: normalized.to_string(),
                base: base_branch.to_string(),
                expected: stored.unwrap_or_else(|| "(none recorded)".to_string()),
                actual: base_sha,
            });
        }
    }

    Ok(base_sha)
}

/// Ensure `.claude/skills/` exist in the target repo directory.
///
/// If `SKILLS_DIR` is set (default `/opt/skills` in the container image),
/// symlinks each skill subdirectory into `repo_path/.claude/skills/`.
/// This makes baked-in skill files available to the LLM subprocess.
///
/// No-op when `SKILLS_DIR` doesn't exist (e.g. local development where
/// skills are already in the working tree).
pub async fn ensure_skills(repo_path: &Path) -> Result<()> {
    let skills_source =
        PathBuf::from(std::env::var("SKILLS_DIR").unwrap_or_else(|_| "/opt/skills".into()));
    let source_skills_dir = skills_source.join(".claude/skills");

    if !source_skills_dir.exists() {
        tracing::debug!(
            path = %source_skills_dir.display(),
            "skills source directory not found, skipping symlink"
        );
        return Ok(());
    }

    let target_skills_dir = repo_path.join(".claude/skills");
    tokio::fs::create_dir_all(&target_skills_dir).await?;

    let mut entries = tokio::fs::read_dir(&source_skills_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let name = entry.file_name();
            let link_path = target_skills_dir.join(&name);
            // Remove existing symlink, file, or directory to ensure a clean link.
            // remove_file handles symlinks and regular files; remove_dir_all
            // handles real directories left by a previous partial run.
            if let Ok(meta) = tokio::fs::symlink_metadata(&link_path).await {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    let _ = tokio::fs::remove_dir_all(&link_path).await;
                } else {
                    let _ = tokio::fs::remove_file(&link_path).await;
                }
            }
            tokio::fs::symlink(&entry_path, &link_path)
                .await
                .map_err(|e| {
                    PipelineError::Enrich(format!(
                        "failed to symlink skill {:?} -> {:?}: {e}",
                        entry_path, link_path
                    ))
                })?;
            tracing::debug!(skill = ?name, "symlinked skill into repo");
        }
    }

    link_schema(&skills_source, repo_path).await;

    Ok(())
}

/// Put the schema where the agent looks for it.
///
/// It writes YAML against a contract with required fields, closed enums and
/// `additionalProperties: false`, so it needs to read that contract. A run in a
/// bare corpus checkout went looking (`ls schema/`, `find . -name schema.json`)
/// and found nothing, then wrote fields the schema forbids.
///
/// Letting the gate catch that afterwards is the wrong division of labour: it
/// makes a check do the work an instruction should have done, and it spends a
/// repair round on something the agent could have known before it started.
///
/// Best effort on purpose. A missing schema is worth a warning and not a failed
/// run, because the skills carry a readable digest of the same contract.
async fn link_schema(source_root: &Path, repo_path: &Path) {
    let source = source_root.join("schema");
    if !source.exists() {
        tracing::warn!(path = %source.display(), "no schema dir to link; the agent will work without the contract");
        return;
    }
    let link = repo_path.join("schema");
    if let Ok(meta) = tokio::fs::symlink_metadata(&link).await {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            return; // A real schema directory is already there.
        }
        let _ = tokio::fs::remove_file(&link).await;
    }
    match tokio::fs::symlink(&source, &link).await {
        Ok(()) => tracing::debug!("symlinked schema into corpus root"),
        Err(e) => tracing::warn!(error = %e, "could not link schema into corpus root"),
    }
}

/// Known absolute prefixes that may appear in yaml_path values from
/// older harvest results. Stripped automatically so enrich jobs still work.
const KNOWN_REPO_PREFIXES: &[&str] = &["/tmp/corpus-repo/", "/tmp/regulation-repo/"];

/// Normalize and validate a yaml_path: strip known absolute prefixes,
/// then verify the path contains only safe characters.
///
/// Prevents path traversal and injection via crafted job payloads.
pub(crate) fn normalize_yaml_path(yaml_path: &str) -> Result<String> {
    if yaml_path.is_empty() {
        return Err(PipelineError::Enrich("yaml_path must not be empty".into()));
    }

    // Auto-strip known absolute prefixes from legacy payloads.
    let mut path = yaml_path.to_string();
    for prefix in KNOWN_REPO_PREFIXES {
        if let Some(stripped) = path.strip_prefix(prefix) {
            tracing::warn!(
                original = %yaml_path,
                normalized = %stripped,
                "yaml_path had absolute prefix, stripped automatically"
            );
            path = stripped.to_string();
            break;
        }
    }

    if path.starts_with('/') {
        return Err(PipelineError::Enrich(format!(
            "yaml_path must be relative, not absolute: {yaml_path}"
        )));
    }
    if !path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'))
    {
        return Err(PipelineError::Enrich(format!(
            "yaml_path contains invalid characters: {path}"
        )));
    }
    if path.contains("..") {
        return Err(PipelineError::Enrich(format!(
            "yaml_path must not contain '..': {path}"
        )));
    }
    Ok(path)
}

/// Error-message marker for a chunk that produced no reviewable output at all:
/// no new `machine_readable` sections, no `chunk_report` in the result
/// envelope, and no new marking or untranslatable. This wording deliberately
/// does NOT contain any `is_deterministic_content_failure` marker ("no machine_readable
/// sections" / "yaml error"): the failure must stay retryable — a healthy law
/// whose chunk merely hiccupped must never be terminally exhausted in one
/// step. The worker's `chunk_no_output_is_not_deterministic` test pins this.
pub(crate) const CHUNK_NO_OUTPUT_MARKER: &str = "enrichment chunk produced no reviewable output";

/// Run the feedback rounds one gate is allowed: evaluate it, and for as long
/// as it has something to say and a round is still worth spending, give the
/// agent a pass to answer.
///
/// The budget comes from `config.feedback_rounds` and is one per gate by
/// default, which is what this did before it was settable. A round beyond the
/// first only earns its place if the previous one moved something, so the
/// chain stops on its own in three ways: the gate is clear, the agent left the
/// file byte-identical, or the finding count did not fall. The last two are
/// the doorbreekvoorwaarde — an unchanged file cannot have removed a finding,
/// and a changed file that removed none is churn or a trade, neither of which
/// gives any reason to expect the next round to do better.
///
/// Returns what every round did, so a run can be read per gate rather than as
/// one number. Findings and markings are recorded side by side, because a
/// falling finding count bought by declaring more of the law unmodellable is
/// the opposite outcome from one bought by translating it.
///
/// `window` is the articles this run is about, and it is passed in rather than
/// read off `payload`. A queue payload never carries one: the worker writes
/// `chunk_articles: None` and the windowed copy exists only inside the branch
/// that runs the agent. Reading it from the payload therefore narrowed nothing
/// on any real job, and handed the gate the whole law while the run had a
/// window. `None` means the whole law, which is what a run without a window is.
async fn run_feedback_rounds(
    gate: Gate,
    yaml_abs: &Path,
    // The corpus checkout. Every caller passed this same path twice, once as
    // the root the gate reads the law against and once as the path the runner
    // works in; they are the same directory and now one parameter.
    corpus_root: &Path,
    payload: &EnrichPayload,
    window: Option<&[String]>,
    config: &EnrichConfig,
    runner: &dyn LlmRunner,
) -> Result<GateFeedback> {
    let reading = evaluate_gate(gate, yaml_abs, corpus_root, window).await?;
    let mut findings = reading.answerable;
    if !reading.outside_corpus.is_empty() {
        tracing::info!(
            gate = gate.label(),
            outside_corpus = reading.outside_corpus.len(),
            first = %reading.outside_corpus.first().cloned().unwrap_or_default(),
            "findings recorded but not put to the agent: nothing in this file answers them"
        );
    }
    let mut progress = GateFeedback {
        gate: gate.label().to_string(),
        findings_initial: findings.len(),
        findings_final: findings.len(),
        rounds: Vec::new(),
        outside_corpus: reading.outside_corpus,
    };
    let budget = config.feedback_rounds.for_gate(gate);
    if findings.is_empty() || budget == 0 {
        return Ok(progress);
    }

    // A round answers findings and runs under its own ceiling, at most the
    // translation's (see `bound_llm_timeout` in the worker).
    let round_config = EnrichConfig {
        timeout: config.feedback_timeout,
        ..config.clone()
    };
    for round in 1..=budget {
        let findings_before = findings.len();
        let markings_before = marking_count(yaml_abs).await;
        let digest_before = file_digest(yaml_abs).await;

        tracing::info!(
            gate = gate.label(),
            round,
            of_rounds = budget,
            findings = findings_before,
            markings = ?markings_before,
            first = %findings.first().cloned().unwrap_or_default(),
            "gate has findings; running a feedback round"
        );

        let feedback_payload = EnrichPayload {
            pass: Pass::Feedback(Feedback {
                gate,
                findings: std::mem::take(&mut findings),
            }),
            ..payload.clone()
        };
        runner
            .run(&feedback_payload, yaml_abs, corpus_root, &round_config)
            .await?;

        let reading = evaluate_gate(gate, yaml_abs, corpus_root, window).await?;
        findings = reading.answerable;
        progress.outside_corpus = reading.outside_corpus;
        let findings_after = findings.len();
        let markings_after = marking_count(yaml_abs).await;
        let file_changed = file_digest(yaml_abs).await != digest_before;

        let stopped = if findings_after == 0 {
            Some(RoundStop::Cleared)
        } else if !file_changed {
            Some(RoundStop::Unchanged)
        } else if findings_after >= findings_before {
            Some(RoundStop::NoDecrease)
        } else if round == budget {
            Some(RoundStop::Budget)
        } else {
            None
        };

        // Findings and markings on one line: whoever reads the log sees both
        // sides of the trade, not only the number that went down.
        tracing::info!(
            gate = gate.label(),
            round,
            findings_before,
            findings_after,
            markings_before = ?markings_before,
            markings_after = ?markings_after,
            file_changed,
            stopped = ?stopped,
            "feedback round finished"
        );

        progress.rounds.push(FeedbackRoundRecord {
            round,
            findings_before,
            findings_after,
            markings_before,
            markings_after,
            file_changed,
            stopped,
        });
        progress.findings_final = findings_after;

        if stopped.is_some() {
            break;
        }
    }

    if findings.is_empty() {
        tracing::info!(gate = gate.label(), "feedback cleared the gate");
        return Ok(progress);
    }

    // A soft gate does not fail the job on what survives. The agent had its
    // chance to answer and what is left is recorded rather than fatal:
    // failing here would turn every open norm into a defect, which is the
    // outcome this design exists to avoid.
    if gate.accepts_marking() {
        tracing::warn!(
            gate = gate.label(),
            remaining = findings.len(),
            rounds = progress.rounds.len(),
            "findings survived the feedback rounds; recorded, not fatal"
        );
        return Ok(progress);
    }

    // The hard gates fail the job, and a failed job commits nothing: the
    // worker only pushes on `Ok`.
    let what = match gate {
        Gate::Binding => "reference(s) to nothing in this law",
        _ => "schema error(s)",
    };
    Err(PipelineError::Enrich(format!(
        "enriched law still has {} {what} after {} feedback round(s): {}",
        findings.len(),
        progress.rounds.len(),
        findings.join("; ")
    )))
}

/// Run the sub-windows of one window side by side, each on its own copy of
/// the checkout, and fold the results back into the real file.
///
/// A copy per agent rather than one shared file. Two agents writing the same
/// YAML is a measurement of which nobody can say afterwards who wrote what,
/// and round 3 lost four runs to exactly that. The copies are disjoint per
/// entry, so the merge has nothing to resolve — and [`merge_windows`] proves
/// that rather than assuming it: a window that touched an entry outside its
/// own assignment fails the run with that entry's number in the message.
async fn run_windows_concurrently(
    sub_windows: &[Vec<String>],
    payload: &EnrichPayload,
    yaml_abs: &Path,
    normalized_path: &str,
    repo_path: &Path,
    config: &EnrichConfig,
    runner: &dyn LlmRunner,
) -> Result<()> {
    let base = tokio::fs::read_to_string(yaml_abs).await?;
    let scratch = tempfile::tempdir()?;

    tracing::info!(
        windows = sub_windows.len(),
        concurrency = config.window_concurrency,
        "one window split over several agents"
    );

    let mut runs = Vec::new();
    for (index, numbers) in sub_windows.iter().enumerate() {
        let checkout = scratch.path().join(format!("window-{index}"));
        copy_tree(repo_path, &checkout).await?;
        let their_yaml = checkout.join(normalized_path);
        let their_payload = EnrichPayload {
            chunk_articles: Some(numbers.clone()),
            // The research belongs to the window as a whole, not to each
            // slice of it: only the first slice is allowed to do it.
            skip_mvt: Some(payload.skip_mvt.unwrap_or(false) || index > 0),
            // Its own session, never the parent's. A session id is the
            // provider's handle on one conversation, so handing the same one
            // to processes running side by side is two agents writing one
            // transcript: the second either refuses to start, adopts the
            // first's history and sees articles it was not assigned, or both
            // write the same session file. Sharing a session across the calls
            // of one window is the point of the feature; sharing it across
            // windows that run at the same time is the opposite of it.
            session: payload
                .session
                .as_ref()
                .map(|parent| std::sync::Arc::new(AgentSession::new(parent.reuse()))),
            ..payload.clone()
        };
        runs.push(async move {
            runner
                .run(&their_payload, &their_yaml, &checkout, config)
                .await?;
            let text = tokio::fs::read_to_string(&their_yaml).await?;
            Ok::<_, PipelineError>(text)
        });
    }

    let texts = futures::future::try_join_all(runs).await?;
    let merged = merge_windows(
        &base,
        &sub_windows.iter().cloned().zip(texts).collect::<Vec<_>>(),
    )
    .map_err(PipelineError::Enrich)?;
    tokio::fs::write(yaml_abs, merged).await?;
    Ok(())
}

/// Recursive copy of a directory tree, for the per-window checkouts.
fn copy_tree<'a>(
    from: &'a Path,
    to: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(to).await?;
        let mut entries = tokio::fs::read_dir(from).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            // A checkout carries its whole history; the agent never reads it
            // and copying it per window is the most expensive thing here.
            if name == ".git" {
                continue;
            }
            let source = entry.path();
            let target = to.join(&name);
            if entry.file_type().await?.is_dir() {
                copy_tree(&source, &target).await?;
            } else {
                tokio::fs::copy(&source, &target).await?;
            }
        }
        Ok(())
    })
}

/// The closing pass over a law whose last window has been walked.
///
/// Two steps, deterministic first. The mechanical bindings —
/// [`crate::enrich_v2::reconcile::plan`] — are written by the worker without
/// asking anybody, because an input with no source whose name is exactly an
/// output another entry declares is not a judgement. Whatever is left is put
/// to one agent through [`Gate::Reconcile`], with a prompt that is mostly a
/// list of what it may not do.
///
/// # Waarom dit niets kan verslechteren
///
/// De wet is hier al door elke poort gegaan, dus alles wat deze pass toevoegt
/// is achteruitgang. Daarom meet de pass zichzelf na, en beide helften doen
/// dat: twee tellingen vóór en ná de schrijfactie, de schemafouten en de
/// deterministische bevindingen. Het is een meting en geen bedoeling: een pass
/// die niets goedmaakt, verdwijnt. Zie [`Rewrite`] voor de twee drempels.
///
/// De drempels verschillen omdat de opdrachten verschillen. De
/// deterministische helft schrijft alleen bindingen waarvan de naam woordelijk
/// klopt, dus die moet iets verbeteren en niets verslechteren; gelijk op beide
/// tellingen gaat terug. De agent-helft krijgt de aanwijzingen die de eerste
/// heeft laten liggen, en of die goed beantwoord zijn valt uit geen van beide
/// tellingen af te lezen: een correct beantwoorde aanwijzing hoeft geen
/// bevinding weg te nemen. Daar is de regel daarom "niets slechter". Loopt een
/// telling op of wordt het bestand onleesbaar, dan gaat de tekst terug naar wat
/// er vóór de agent stond. Zonder die terugdraaiing was dit de enige schrijver
/// in de hele keten zonder net, en de laatste bovendien.
async fn run_closing_reconcile(
    yaml_abs: &Path,
    corpus_root: &Path,
    payload: &EnrichPayload,
    config: &EnrichConfig,
    runner: &dyn LlmRunner,
) -> Result<Vec<GateFeedback>> {
    let before_text = tokio::fs::read_to_string(yaml_abs).await?;
    let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&before_text) else {
        tracing::warn!(law = %yaml_abs.display(), "closing pass skipped: the file is not YAML");
        return Ok(Vec::new());
    };
    let plan = crate::enrich_v2::reconcile::plan(&doc);

    if !plan.links.is_empty() {
        let (after_text, written) = crate::enrich_v2::reconcile::apply(&before_text, &plan.links);
        let rewrite = Rewrite::measure(&before_text, &after_text, corpus_root);
        if rewrite.is_progress() {
            tokio::fs::write(yaml_abs, &after_text).await?;
            tracing::info!(
                law = %yaml_abs.display(),
                bound = written.len(),
                findings_before = rewrite.findings_before,
                findings_after = rewrite.findings_after,
                links = %written
                    .iter()
                    .map(crate::enrich_v2::reconcile::Link::describe)
                    .collect::<Vec<_>>()
                    .join("; "),
                "closing pass connected what already existed"
            );
        } else {
            tracing::warn!(
                law = %yaml_abs.display(),
                planned = plan.links.len(),
                findings_before = rewrite.findings_before,
                findings_after = rewrite.findings_after,
                readable = rewrite.readable,
                schema_before = rewrite.schema_before,
                schema_after = rewrite.schema_after,
                "closing pass reverted: it did not improve the count"
            );
        }
    }

    // The leads go to an agent only when there are any; the gate evaluates
    // them again itself, so a lead the deterministic write just resolved
    // never reaches a prompt.
    let before_agent = tokio::fs::read_to_string(yaml_abs).await?;
    let progress = run_feedback_rounds(
        Gate::Reconcile,
        yaml_abs,
        corpus_root,
        payload,
        // The closing pass runs once the last window has been walked, so it is
        // about the whole law by construction.
        None,
        config,
        runner,
    )
    .await?;
    revert_agent_half_if_worse(yaml_abs, corpus_root, &before_agent).await?;
    Ok(vec![progress])
}

/// What one rewrite of a whole law did to the deterministic measurements.
///
/// Both halves of the closing pass decide on the same three numbers, and both
/// used to compute them in place. Naming them makes the decision itself a
/// thing that can be handed a before and an after and asked what it would do,
/// which is the only way the refusing branch gets covered: the accepting
/// branch is easy to reach through the real planner and the refusing one is
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rewrite {
    findings_before: usize,
    findings_after: usize,
    schema_before: usize,
    schema_after: usize,
    readable: bool,
}

impl Rewrite {
    fn measure(before: &str, after: &str, corpus_root: &Path) -> Self {
        let report_before = crate::enrich_v2::checks::run(before, Some(corpus_root));
        let report_after = crate::enrich_v2::checks::run(after, Some(corpus_root));
        Self {
            findings_before: report_before.findings.len(),
            findings_after: report_after.findings.len(),
            schema_before: report_before.schema.len(),
            schema_after: report_after.schema.len(),
            readable: serde_yaml_ng::from_str::<serde_yaml_ng::Value>(after).is_ok(),
        }
    }

    /// Nothing the pass measures got worse.
    ///
    /// `readable` is asked separately and not read off `schema_after`, because
    /// an unreadable file is reported as exactly one schema error — the parse
    /// diagnosis — and on a law that already carried three that would count as
    /// an improvement.
    fn is_not_worse(self) -> bool {
        self.readable
            && self.schema_after <= self.schema_before
            && self.findings_after <= self.findings_before
    }

    /// The threshold of the deterministic half: nothing worse, and at least
    /// one of the two measurements better.
    ///
    /// Equal on both is a refusal, and not out of strictness. An equal count
    /// is not an equal set — one finding that disappears beside one that
    /// appears counts the same — so a write that traded one defect for another
    /// would go through under the log line saying it connected what already
    /// existed.
    ///
    /// The schema count is in the rule because that is where this half's work
    /// actually lands. An input without `source` is a schema error and not a
    /// checks finding, so binding it repairs the schema while the finding
    /// count stands still. Measured on the finding count alone the pass never
    /// improves anything and would refuse its own only effect — which is what
    /// a rule of "the count has to fall" does here, and what the fixture in
    /// [`tests::the_closing_pass_binds_what_the_window_could_not_see`] shows:
    /// zero findings before and after, one schema error repaired.
    fn is_progress(self) -> bool {
        self.is_not_worse()
            && (self.findings_after < self.findings_before
                || self.schema_after < self.schema_before)
    }
}

/// Put the law back the way the agent found it when its edit made things
/// worse.
///
/// [`run_feedback_rounds`] is a stopping rule and not a rollback:
/// [`RoundStop::NoDecrease`] ends the loop and leaves standing whatever the
/// agent wrote. For every other gate that is survivable, because a hard schema
/// gate runs after them over the same file. This gate is the last writer of
/// the run, so what it leaves is what gets committed, and the guarantee
/// RFC-033 states over "de pass" has to hold over this half too.
///
/// Worse means one of three things, in the order they cost: the file no longer
/// parses, the schema refuses it, or the deterministic checks have more to say
/// than before. Equal is not worse here — see the doc of
/// [`run_closing_reconcile`] for why the two halves differ on that.
async fn revert_agent_half_if_worse(
    yaml_abs: &Path,
    corpus_root: &Path,
    before_agent: &str,
) -> Result<()> {
    let after_text = tokio::fs::read_to_string(yaml_abs).await?;
    if after_text == before_agent {
        return Ok(());
    }
    let rewrite = Rewrite::measure(before_agent, &after_text, corpus_root);
    if rewrite.is_not_worse() {
        return Ok(());
    }
    tokio::fs::write(yaml_abs, before_agent).await?;
    tracing::warn!(
        law = %yaml_abs.display(),
        findings_before = rewrite.findings_before,
        findings_after = rewrite.findings_after,
        readable = rewrite.readable,
        schema_before = rewrite.schema_before,
        schema_after = rewrite.schema_after,
        "closing pass reverted the agent's edit: it left the law worse than it found it"
    );
    Ok(())
}

/// Markings recorded in the law as it stands, or `None` when the file is not
/// even YAML.
///
/// Counted off the untyped tree rather than the law model, and that is the
/// whole point of the function. It used to go through [`load_law`], which
/// deserialises the file into `ArticleBasedLaw` and fails on anything the
/// model does not yet have a shape for. In the measured run the agent's very
/// first edit wrote `requires:` as a list of mappings while the model had it
/// as a list of strings, so every call returned `None` from the first round
/// on, and the log recorded `markings_before=None markings_after=None` for
/// every feedback round of the run. That is exactly the counter meant to catch
/// a falling finding count bought by declaring more of the law unmodellable,
/// which is the trade round 3 made, and it was blind for the whole run.
///
/// A counter is not a validator. Whether the file conforms to the model is the
/// schema gate's question and it answers it out loud; this one only has to say
/// how many markings are written down, and a shape it does not recognise
/// elsewhere in the file is no reason to stop knowing that. `None` therefore
/// now means what it says: the file could not be read at all.
async fn marking_count(yaml_abs: &Path) -> Option<usize> {
    let raw = tokio::fs::read_to_string(yaml_abs).await.ok()?;
    let doc: serde_yaml_ng::Value = match serde_yaml_ng::from_str(&raw) {
        Ok(doc) => doc,
        Err(error) => {
            // Loud, because "unknown" is a real answer here and a silent one
            // is how this went unnoticed for a whole run.
            tracing::warn!(law = %yaml_abs.display(), %error, "cannot count markings: file is not YAML");
            return None;
        }
    };
    Some(count_markings_in(&doc))
}

/// Markings in an untyped law document.
fn count_markings_in(doc: &serde_yaml_ng::Value) -> usize {
    doc.get("articles")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .map(|articles| {
            articles
                .iter()
                .filter_map(|article| {
                    article
                        .get("machine_readable")?
                        .get("markings")?
                        .as_sequence()
                        .map(Vec::len)
                })
                .sum()
        })
        .unwrap_or(0)
}

/// What a gate says about the law as it stands, split by who can answer it.
#[derive(Debug, Default, Clone)]
struct GateReading {
    /// Findings the agent can answer by editing this file.
    answerable: Vec<String>,
    /// Findings whose resolution lies outside this file entirely: the law it
    /// binds to is not in the corpus, or is there and carries no model yet.
    outside_corpus: Vec<String>,
}

/// What a gate says about the law as it stands.
///
/// The split is the point. Round 5 put nineteen checks findings to the agent
/// and sixteen survived two rounds; twelve of those sixteen were of the form
/// `"zorgverzekeringswet" does not produce output "is_verzekerde"`, against a
/// corpus whose laws carry no model at all. No edit to the reading file can
/// make another law produce an output, so those rounds could only ever end in
/// the same list coming back unchanged. Asking anyway is not merely wasted: it
/// teaches the agent that answering is not what the gate wants, and the way out
/// it finds is to invent one.
///
/// They still have to be recorded — a binding onto a law nobody has harvested
/// is work, just not this agent's — so they leave the loop and land in the run
/// result instead.
/// Whether a finding about `article` falls inside the window this run owns.
///
/// The prompt tells the agent to leave every article outside its list
/// completely untouched, and the gate ran over the whole file, so a finding
/// about article 3 in a window of 1 and 2 is an instruction to break the one
/// rule the chunking rests on. Round 5 measured both halves of that: under the
/// checks gate the agent declared four such findings out of scope and they came
/// back unchanged twice, and under the marking gate's wording it modelled six
/// out-of-window articles instead — correct work, done by the run that did not
/// own it, on a file another run may be editing.
///
/// So the window filters the findings rather than the prompt explaining the
/// contradiction away. Every article reaches a window eventually; the finding
/// belongs to that run.
///
/// Matched in both directions because window and finding may be at different
/// granularities: a window entry `3.2` is inside article `3`, and a finding
/// about entry `3.2` is inside a window of `3`.
/// The article number a schema error is about, when it is about one.
///
/// Validation errors are formatted `"{instance_path}: {message}"`, and the
/// path of anything inside an article starts `/articles/{index}`. The index is
/// a position in the sequence, so it has to be resolved against the file to
/// become the number an agent knows the article by.
///
/// `None` for an error about the document as a whole, which stays in every
/// window: a missing `$schema` is nobody's article and everybody's problem.
fn schema_error_article(raw: &str, error: &str) -> Option<String> {
    let index: usize = error
        .strip_prefix("/articles/")?
        .split(['/', ':'])
        .next()?
        .parse()
        .ok()?;
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(raw).ok()?;
    doc.get("articles")?
        .as_sequence()?
        .get(index)?
        .get("number")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_string)
}

/// The entries whose content differs between two versions of a law, by
/// number, including entries only the second one has. Empty when either
/// version does not parse: the caller then learns nothing it can narrow on,
/// and the schema gate that reads the file will say why.
fn changed_entries(before: &str, after: &str) -> Vec<String> {
    fn entries(raw: &str) -> Option<Vec<(String, serde_yaml_ng::Value)>> {
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(raw).ok()?;
        Some(
            doc.get("articles")?
                .as_sequence()?
                .iter()
                .filter_map(|article| {
                    let number = match article.get("number")? {
                        serde_yaml_ng::Value::String(s) => s.clone(),
                        other => serde_yaml_ng::to_string(other).ok()?.trim().to_string(),
                    };
                    Some((number, article.clone()))
                })
                .collect(),
        )
    }
    let (Some(before), Some(after)) = (entries(before), entries(after)) else {
        return Vec::new();
    };
    after
        .into_iter()
        .filter(|(number, entry)| {
            !before
                .iter()
                .any(|(was_number, was)| was_number == number && was == entry)
        })
        .map(|(number, _)| number)
        .collect()
}

fn in_window(window: Option<&[String]>, article: Option<&str>) -> bool {
    let (Some(window), Some(article)) = (window, article) else {
        return true;
    };
    window.iter().any(|entry| {
        entry == article
            || article.starts_with(&format!("{entry}."))
            || entry.starts_with(&format!("{article}."))
    })
}

async fn evaluate_gate(
    gate: Gate,
    yaml_abs: &Path,
    corpus_root: &Path,
    window: Option<&[String]>,
) -> Result<GateReading> {
    let raw = tokio::fs::read_to_string(yaml_abs).await?;
    let mut reading = GateReading::default();
    match gate {
        Gate::Schema => {
            reading.answerable = crate::enrich_v2::checks::schema_errors(&raw)
                .into_iter()
                .filter(|error| in_window(window, schema_error_article(&raw, error).as_deref()))
                .collect();
        }
        // The closing gate reads the whole law and never the window: the
        // whole point is that it looks at what every earlier window left.
        //
        // Parsing comes first here too, and when it fails it comes alone. An
        // empty finding list is what this loop reads as a clean gate, so a
        // file the agent made unreadable used to report itself as "nothing
        // left to do" — the same silence [`crate::enrich_v2::checks::run`] was
        // given a `parses` finding for. The other three arms of this match
        // already say it out loud; this one was the exception.
        Gate::Reconcile => match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&raw) {
            Ok(doc) => {
                reading.answerable = crate::enrich_v2::reconcile::plan(&doc)
                    .leads
                    .iter()
                    .map(crate::enrich_v2::reconcile::Lead::describe)
                    .collect();
            }
            Err(e) => {
                reading.answerable = vec![crate::enrich_v2::checks::parse_diagnosis(&raw, &e)];
            }
        },
        // Whole-law for the same reason as the closing pass it follows: the
        // reference and what it resolves to may sit in different windows.
        Gate::Binding => match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&raw) {
            Ok(doc) => {
                reading.answerable = crate::enrich_v2::checks::dangling_bindings(&doc)
                    .into_iter()
                    .map(|finding| match &finding.article {
                        Some(number) => {
                            format!("[{}] art. {number}: {}", finding.check, finding.detail)
                        }
                        None => format!("[{}] {}", finding.check, finding.detail),
                    })
                    .collect();
            }
            Err(e) => {
                reading.answerable = vec![crate::enrich_v2::checks::parse_diagnosis(&raw, &e)];
            }
        },
        Gate::Marking | Gate::Checks => {
            for finding in crate::enrich_v2::checks::run_with_companions(
                &raw,
                Some(corpus_root),
                yaml_abs.parent(),
                Some(yaml_abs),
            )
            .findings
            {
                // `accounted` asks whether the article carries any outcome at
                // all, and the answer may be a marking. That makes it a question
                // for the soft gate, beside the other two, rather than a defect.
                let is_record = matches!(
                    finding.check,
                    "marking" | "citation" | "accounted" | "reference"
                );
                if matches!(gate, Gate::Marking) != is_record {
                    continue;
                }
                if !in_window(window, finding.article.as_deref()) {
                    continue;
                }
                let line = match &finding.article {
                    Some(number) => {
                        format!("[{}] art. {number}: {}", finding.check, finding.detail)
                    }
                    None => format!("[{}] {}", finding.check, finding.detail),
                };
                // `corpus-defect` joins `outside-corpus` here for the same
                // reason and not for the same fact: both are about another
                // file, so nothing in this one answers them. They stay apart
                // in the record because the work differs — the one is
                // harvested or interpreted, the other repaired.
                if matches!(finding.check, "outside-corpus" | "corpus-defect") {
                    reading.outside_corpus.push(line);
                } else {
                    reading.answerable.push(line);
                }
            }
        }
    }
    Ok(reading)
}

/// Content fingerprint of a file, or `None` when it cannot be read. Only
/// used to decide whether a run changed anything.
async fn file_digest(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = tokio::fs::read(path).await.ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

/// Execute the enrichment using the default process-based LLM runner.
///
/// The entries of one article, in document order.
///
/// The harvest splits below article level, so article `69` of the
/// Zorgverzekeringswet is `69.1` through `69.17` with sub-items under those,
/// and `2.1.b` of the Awir is an item of article 2. An article is the unit a
/// reader and a lawyer both recognise; an entry is a rendering choice of the
/// harvester.
///
/// Matches the entry itself and anything the harvest hung under it, on the
/// separator rather than on the prefix, so `69` does not take `690`.
fn entries_of(entry_numbers: &[String], article: &str) -> Vec<String> {
    let with_dot = format!("{article}.");
    entry_numbers
        .iter()
        .filter(|n| n.as_str() == article || n.starts_with(&with_dot))
        .cloned()
        .collect()
}

/// Convenience wrapper around `execute_enrich_with_runner` using `ProcessLlmRunner`.
pub async fn execute_enrich(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
    source_hash: &str,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    execute_enrich_with_runner(payload, repo_path, config, source_hash, &ProcessLlmRunner).await
}

/// Execute the enrichment: call the LLM runner to generate machine_readable sections.
///
/// Returns the enrichment result and a list of files that were written
/// (for git staging). Accepts a `runner` to allow testing with a fake LLM.
pub async fn execute_enrich_with_runner(
    payload: &EnrichPayload,
    repo_path: &Path,
    config: &EnrichConfig,
    source_hash: &str,
    runner: &dyn LlmRunner,
) -> Result<(EnrichResult, Vec<PathBuf>)> {
    let normalized_path = normalize_yaml_path(&payload.yaml_path)?;

    // One session for this window, opened by the translation pass and
    // continued by whichever feedback rounds the mode allows. It is created
    // here and dropped when this call returns, which is exactly what "the
    // session ends with the window" means: nothing carries it to the next
    // chunk of the same law.
    let session = std::sync::Arc::new(AgentSession::new(config.session_reuse));
    let payload = &EnrichPayload {
        session: Some(std::sync::Arc::clone(&session)),
        ..payload.clone()
    };

    let yaml_abs = repo_path.join(&normalized_path);
    if !yaml_abs.exists() {
        return Err(PipelineError::Enrich(format!(
            "law YAML file not found: {}",
            yaml_abs.display()
        )));
    }

    // Fingerprint before the run, so the schema gate below can tell an
    // enrichment that produced something from one that did not.
    let digest_before = file_digest(&yaml_abs).await;

    // Parse the law once for the pre-run stats, the chunk window's article
    // numbers, and the recorded-gap baseline of the chunk no-op guard.
    let law = load_law(&yaml_abs).await?;
    let (articles_before, machine_readable_before) = article_stats(&law);
    let texts_before = article_texts(&law);

    // Chunk planning: the worker (not the LLM) owns the cursor, read from the
    // `.enrichment.yaml` already present on the enrich branch checkout.
    let (stored_cursor, stored_cursor_path, stored_cursor_mode) =
        read_stored_cursor(repo_path, &normalized_path).await;
    // A cursor recorded under another window mode counts entries where this
    // run counts layers, or the other way round. Reading it as if it meant the
    // same thing would silently skip or repeat work, so a mode change resets
    // the walk — the same rule a path change already follows.
    let stored_cursor_path =
        if stored_cursor_mode.is_empty() || stored_cursor_mode == config.window_mode.label() {
            stored_cursor_path
        } else {
            tracing::info!(
                was = %stored_cursor_mode,
                now = %config.window_mode.label(),
                "window mode changed; the walk restarts at the beginning of the law"
            );
            String::new()
        };
    // A named entry overrides the cursor: this run enriches that one entry and
    // nothing else. Targeted work is not progress through the document, so the
    // cursor stands still — a run that repairs one article must not push the
    // ordinary walk past articles no agent has seen. The termination property
    // of the cursor mode is therefore untouched: it still advances only when a
    // window is walked, in `ceil(total / N)` successful runs.
    let targeted = !config.target_articles.is_empty();
    // Entry numbers in document order, so the window boundary can keep a
    // top-level article together (see `plan_chunk`).
    let entry_numbers: Vec<String> = law.articles.iter().map(|a| a.number.clone()).collect();
    let (chunk_window, law_complete, next_cursor) = match targeted {
        true => {
            // Articles, not entries. The harvest splits below article level
            // (the Zorgverzekeringswet has 742 entries over far fewer
            // articles), so `--article 69` names 22 of them, and asking for one
            // at a time would pay the fixed per-session cost 22 times for one
            // article. `entries_of` takes the article and everything the
            // harvest hung under it.
            //
            // More than one article arrives here from `--depth`: the closure
            // plans a set of articles per law, and enriching them in one window
            // pays the session cost once instead of once per article.
            let mut numbers: Vec<String> = Vec::new();
            for article in &config.target_articles {
                let found = entries_of(&entry_numbers, article);
                if found.is_empty() {
                    // Loud on purpose: naming an entry that is not there is a
                    // mistake in the caller's query, and a run that quietly
                    // enriched nothing would look like a run that found
                    // nothing to do.
                    return Err(PipelineError::Enrich(format!(
                        "law {normalized_path} has no article {article}; it has \
                         {articles_before} entries and this run enriches nothing else"
                    )));
                }
                numbers.extend(found);
            }
            // Document order, so the window reads as the law reads.
            numbers.sort_by_key(|n| entry_numbers.iter().position(|e| e == n));
            let index = numbers
                .first()
                .and_then(|first| entry_numbers.iter().position(|n| n == first))
                .unwrap_or(0);
            // What the ordinary walk had reached, under the same reset rule
            // `plan_chunk` applies: a cursor recorded for another path or
            // beyond the document does not survive into this run's metadata.
            let cursor_now =
                if stored_cursor_path == normalized_path && stored_cursor <= articles_before {
                    stored_cursor
                } else {
                    0
                };
            // A targeted run claims no completion it did not achieve. It only
            // carries forward a walk that had already finished.
            let walk_finished = config.max_articles_per_run > 0 && cursor_now >= articles_before;
            (Some((index, numbers)), walk_finished, cursor_now)
        }
        // A window derived from the law instead of counted off the document:
        // one layer of the reference graph, whose members do not depend on one
        // another and can therefore be translated in any order among
        // themselves. The cursor counts layers here, so the walk still ends in
        // a fixed number of runs.
        false if config.window_mode == WindowMode::Layer && config.max_articles_per_run > 0 => {
            let raw = tokio::fs::read_to_string(&yaml_abs).await?;
            let graph = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&raw)
                .map(|doc| crate::enrich_v2::refgraph::Graph::scan(&doc))
                .unwrap_or_default();
            let layer_index =
                if stored_cursor_path == normalized_path && stored_cursor <= graph.layers().len() {
                    stored_cursor
                } else {
                    0
                };
            let (numbers, complete) = plan_layer_window(&graph, &entry_numbers, layer_index);
            let start = numbers
                .first()
                .and_then(|n| entry_numbers.iter().position(|e| e == n))
                .unwrap_or(0);
            (Some((start, numbers)), complete, layer_index + 1)
        }
        false => {
            let plan = plan_chunk(
                config.max_articles_per_run,
                articles_before,
                stored_cursor,
                &stored_cursor_path,
                &normalized_path,
                &entry_numbers,
            );
            match plan {
                ChunkPlan::WholeLaw => (None, true, 0),
                ChunkPlan::Chunk {
                    start,
                    end,
                    law_complete,
                } => {
                    let numbers: Vec<String> = law.articles[start..end]
                        .iter()
                        .map(|a| a.number.clone())
                        .collect();
                    (Some((start, numbers)), law_complete, end)
                }
            }
        }
    };
    // A run that walks no window moves no cursor.
    //
    // `--steps reconcile` skips the translation and runs only the closing pass,
    // and the cursor the branches above computed is the cursor of the window
    // that would have been walked. Writing it would push the ordinary walk past
    // articles no agent has seen — the same thing a targeted run must not do,
    // and for the same reason (RFC-026, "De cursor blijft staan"). The walk
    // still ends in a fixed number of runs: the cursor advances only when a
    // window is walked.
    //
    // The stored cursor carries over under the rule `plan_chunk` applies: one
    // recorded for another path, or past the end of this document, is not this
    // law's progress and resets to the beginning.
    let next_cursor = if config.steps.window {
        next_cursor
    } else if stored_cursor_path == normalized_path && stored_cursor <= articles_before {
        stored_cursor
    } else {
        0
    };
    // Window-scoped baseline for the chunk no-op guard: progress is measured
    // inside the assigned window only.
    let window_stats_before = chunk_window
        .as_ref()
        .map(|(_, numbers)| window_progress_stats(&law, numbers));

    let provider_name = config.provider.name().to_string();

    tracing::info!(
        law_id = %payload.law_id,
        yaml_path = %payload.yaml_path,
        provider = %provider_name,
        articles = articles_before,
        already_enriched = machine_readable_before,
        chunk = ?chunk_window.as_ref().map(|(start, numbers)| (*start, numbers.len())),
        "starting enrichment"
    );

    // An empty window (valid cursor already at the end of the document) means
    // the chunk loop finished this law earlier: nothing to process, no LLM run
    // — complete trivially instead of prompting an agent with zero articles.
    let empty_window =
        matches!(&chunk_window, Some((_, numbers)) if numbers.is_empty()) || !config.steps.window;
    if empty_window {
        tracing::info!(
            law_id = %payload.law_id,
            cursor = stored_cursor,
            "chunk cursor already at end of document; completing without an LLM run"
        );
    } else {
        // A previous chunk's committed envelope must not serve as
        // proof-of-review for this window (see clear_stale_chunk_report).
        if chunk_window.is_some() {
            clear_stale_chunk_report(&yaml_abs).await;
        }
        let normalized_payload = EnrichPayload {
            pass: Pass::Translate,
            yaml_path: normalized_path.clone(),
            chunk_articles: chunk_window.as_ref().map(|(_, numbers)| numbers.clone()),
            // MvT research runs once, during the first chunk (cursor 0). A
            // targeted run never does it: it repairs one entry in a law the
            // research has already been done for, and redoing it would be the
            // most expensive part of a run meant to be cheap.
            skip_mvt: chunk_window
                .as_ref()
                .map(|(start, _)| targeted || *start > 0),
            ..payload.clone()
        };
        let sub_windows = match (&chunk_window, config.window_concurrency) {
            (Some((_, numbers)), n) if n > 1 => {
                let raw = tokio::fs::read_to_string(&yaml_abs).await?;
                let graph = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&raw)
                    .map(|doc| crate::enrich_v2::refgraph::Graph::scan(&doc))
                    .unwrap_or_default();
                split_window(&graph, numbers, n)
            }
            _ => Vec::new(),
        };
        if sub_windows.len() > 1 {
            run_windows_concurrently(
                &sub_windows,
                &normalized_payload,
                &yaml_abs,
                &normalized_path,
                repo_path,
                config,
                runner,
            )
            .await?;
        } else {
            runner
                .run(&normalized_payload, &yaml_abs, repo_path, config)
                .await?;
        }

        tracing::info!(law_id = %payload.law_id, provider = %provider_name, "enrichment completed");
    }
    let file_changed_this_run = file_digest(&yaml_abs).await != digest_before;

    // Schema validation with one repair round, but only over what this run
    // added. The `law-generate` skill instructs a `just validate` loop and
    // the agent has no shell, no `Justfile` and no `schema/` in its
    // checkout, so that loop cannot run and nothing establishes whether the
    // output is schema-valid. Doing it here needs none of those, because
    // the engine's validator is a library call.
    //
    // A run that changed nothing is not this run's business: the file was
    // already in whatever state it was in, and validating it here would
    // fail a job for someone else's defect.
    // Per gate, in order: the hard gate first, because a law that does not
    // validate cannot be meaningfully asked about coverage and the checks
    // would report noise over a broken tree; the marking gate last, because
    // the round before it produces markings and this one is about where they
    // landed.
    // What this run was about, for the gates to judge. A gate that reports on
    // articles the run never touched blames this job for someone else's
    // defect, which is the same reason `file_changed_this_run` guards the
    // block below.
    let gate_window: Option<Vec<String>> = chunk_window
        .as_ref()
        .map(|(_, numbers)| numbers.clone())
        .filter(|numbers| !numbers.is_empty());

    let mut feedback = Vec::new();
    if file_changed_this_run {
        for gate in Gate::ALL {
            feedback.push(
                run_feedback_rounds(
                    gate,
                    &yaml_abs,
                    repo_path,
                    payload,
                    gate_window.as_deref(),
                    config,
                    runner,
                )
                .await?,
            );
        }
    }

    // The closing pass, once the walk has reached the end of the document.
    //
    // Not conditional on this run having changed anything: what it looks for
    // was left by the windows before it, and a last window that added nothing
    // does not make those earlier bindings any less connectable.
    let digest_before_closing = file_digest(&yaml_abs).await;
    if (law_complete || !config.steps.window) && config.steps.reconcile {
        feedback
            .extend(run_closing_reconcile(&yaml_abs, repo_path, payload, config, runner).await?);
    }
    let closing_pass_wrote = file_digest(&yaml_abs).await != digest_before_closing;

    // The binding gate, over the whole law, once the walk is over and the
    // closing pass has connected what it could (#1036).
    //
    // Under the checks gate a reference to nothing is soft, and has to be: a
    // window may read an output that a later window has yet to write. Once
    // the last window is behind us that excuse is gone, and what still
    // resolves to nothing is a law that is schema-valid and fails the moment
    // it runs. So it gets a repair round and then fails the job, exactly like
    // the schema gate, and a failed job commits nothing.
    //
    // Not conditional on this run having written: the earlier windows wrote
    // and committed, and this is the run that declares the law finished.
    //
    // What the repair writes is folded into the window of the final schema
    // gate below. The finding may sit in an entry of an earlier window, and a
    // schema gate that looked only at this run's window would let the repair
    // break the file where nobody looks.
    //
    // "Over" means every entry has been walked, which is stricter than the
    // condition the closing pass runs under. `--steps reconcile` runs that
    // pass wherever the cursor stands, and on a law still being walked a
    // forward reference is not yet a defect. Such a run only gets the gate
    // when the cursor it leaves standing is at the end of the document; a
    // layer walk counts layers rather than entries, so there it is left to
    // the run that walks the last layer.
    let walk_finished = if config.steps.window || targeted {
        law_complete
    } else if chunk_window.is_none() {
        true
    } else if config.window_mode == WindowMode::Document {
        next_cursor >= articles_before
    } else {
        false
    };
    let mut schema_final_window = gate_window.clone();
    let mut binding_pass_wrote = false;
    if walk_finished {
        let before_binding = tokio::fs::read_to_string(&yaml_abs).await?;
        let mut binding = run_feedback_rounds(
            Gate::Binding,
            &yaml_abs,
            repo_path,
            payload,
            None,
            config,
            runner,
        )
        .await?;
        binding.gate = "binding-final".to_string();
        feedback.push(binding);
        let after_binding = tokio::fs::read_to_string(&yaml_abs).await?;
        if after_binding != before_binding {
            binding_pass_wrote = true;
            if let Some(window) = schema_final_window.as_mut() {
                for number in changed_entries(&before_binding, &after_binding) {
                    if !window.contains(&number) {
                        window.push(number);
                    }
                }
            }
        }
    }

    // The hard gate again, after every writer of the run and not just after
    // the two soft gates.
    //
    // Round 5 measured the cost of not doing this. The schema gate passed,
    // the marking round then added an `overrides` entry without its `law`
    // key, and nothing looked at the file again until `load_law` below,
    // by which time the session was gone. The one agent that could have
    // fixed it in a sentence — the one that had just written it, with the
    // article still in front of it — was never asked. On a clean file this
    // costs one evaluation and no agent call at all.
    //
    // That repair used to sit one writer too early: it ran before the closing
    // pass, so the reconcile agent — the last hand on the file — wrote behind
    // the last schema gate. What stood after it was `load_law`, and the model
    // is on documented points wider than the schema (see `KNOWN_GAPS`): a
    // marking without `resolved_by` is refused by the schema and accepted by
    // the model, so a schema-invalid law could be committed off a run that
    // reported success. The gate belongs after the last writer, which is what
    // it now is.
    if file_changed_this_run || closing_pass_wrote || binding_pass_wrote {
        let mut closing = run_feedback_rounds(
            Gate::Schema,
            &yaml_abs,
            repo_path,
            payload,
            schema_final_window.as_deref(),
            config,
            runner,
        )
        .await?;
        // Named apart from the opening pass so a reader of the record can tell
        // which of the two found what.
        closing.gate = "schema-final".to_string();
        feedback.push(closing);
    }

    // The schema repair above is the last writer, and it may rename what the
    // binding gate just saw resolve. No agent round for that: the check is
    // deterministic and costs one read, and a law that fails it now fails
    // the job rather than landing.
    if walk_finished {
        let reading = evaluate_gate(Gate::Binding, &yaml_abs, repo_path, None).await?;
        if !reading.answerable.is_empty() {
            return Err(PipelineError::Enrich(format!(
                "enriched law still has {} reference(s) to nothing in this law after the \
                 final schema repair: {}",
                reading.answerable.len(),
                reading.answerable.join("; ")
            )));
        }
    }

    // Count articles with machine_readable after enrichment.
    // Coverage score measures what the LLM *added* this session, not total coverage.
    let law_after = load_law(&yaml_abs).await?;
    let (articles_after, articles_with_machine_readable) = article_stats(&law_after);
    if articles_after != articles_before {
        return Err(PipelineError::Enrich(format!(
            "article count changed during enrichment (before={articles_before}, after={articles_after}) — LLM modified YAML structure"
        )));
    }
    // The words of the law are not the translator's to change. Reported with
    // the article number and both lengths, because the usual shape is a text
    // quietly truncated rather than rewritten, and a diff of two long strings
    // buries that.
    let texts_after = article_texts(&law_after);
    if let Some((number, before)) = texts_before
        .iter()
        .find(|(number, before)| texts_after.get(*number) != Some(*before))
    {
        let after_len = texts_after.get(number).map_or(0, String::len);
        return Err(PipelineError::Enrich(format!(
            "the statutory text of article {number} changed during enrichment \
             (was {} characters, now {after_len}) — a translation may add a model, \
             never edit the law's own words",
            before.len()
        )));
    }
    let newly_enriched = articles_with_machine_readable.saturating_sub(machine_readable_before);
    let articles_needing_enrichment = articles_before.saturating_sub(machine_readable_before);
    let coverage_score = if articles_needing_enrichment > 0 {
        newly_enriched as f64 / articles_needing_enrichment as f64
    } else if articles_before > 0 {
        // All articles already had machine_readable before — nothing to do
        1.0
    } else {
        0.0
    };

    // Read the result envelope the agent may have written: related legislation
    // plus (chunked) the chunk_report used by the no-op guard below. Never
    // fails: absent/malformed → default (see read_enrichment_result_envelope).
    let envelope = read_enrichment_result_envelope(&yaml_abs).await;

    // Capture what the agent flagged in the enriched YAML: untranslatables
    // (RFC-012, schema v0.5.x) and the markings that replaced them in v0.7.0.
    let untranslatables = collect_untranslatables_from(&law_after);
    let markings = collect_markings_from(&law_after);

    match &chunk_window {
        // Whole-law mode: if the LLM ran successfully but didn't enrich any
        // articles, treat it as an error so the job gets retried or marked as
        // failed instead of silently committing a zero-coverage result.
        // Unchanged from the pre-chunking behavior (and deliberately matched
        // by `is_deterministic_content_failure` in the worker).
        None => {
            if articles_needing_enrichment > 0 && newly_enriched == 0 {
                return Err(PipelineError::Enrich(format!(
                    "LLM produced no machine_readable sections ({articles_needing_enrichment} articles needed enrichment)"
                )));
            }
        }
        // Chunked mode: a window may legitimately yield zero new
        // machine_readable sections (definition/transitional chapters) — but
        // only when the agent proves it reviewed the window (chunk_report) or
        // recorded a new gap in it (a marking or an untranslatable). No output
        // at all fails with a message that deliberately does NOT match
        // `is_deterministic_content_failure`: the failure stays retryable and
        // can never terminally exhaust a healthy law. The empty window skipped
        // the LLM, so it is exempt.
        Some((start, numbers)) if !empty_window => {
            // Progress is measured INSIDE the assigned window: an edit outside
            // `[start, end)` (a prompt violation, though it rides along in the
            // commit exactly as it would in whole-law mode) must not count as
            // proof that this window was reviewed — otherwise the cursor
            // advances past a window nobody looked at.
            let (win_enriched_before, win_gaps_before) = window_stats_before.unwrap_or((0, 0));
            let (win_enriched_after, win_gaps_after) = window_progress_stats(&law_after, numbers);
            let window_newly_enriched = win_enriched_after.saturating_sub(win_enriched_before);
            // A window of already-modelled articles that this run only flagged
            // is reviewed work, not a no-op: a new marking counts the same as a
            // new untranslatable.
            let window_new_gaps = win_gaps_after > win_gaps_before;
            // The chunk_report only counts as proof-of-review when it names at
            // least one article of THIS window: a bare `chunk_report: {}` or
            // one listing unrelated numbers must not advance the cursor past
            // an unreviewed window. Full window coverage is deliberately NOT
            // required — exact-match strictness against agent-written numbers
            // could retry-loop a healthy chunk toward exhaustion.
            let report_references_window = envelope.chunk_report.as_ref().is_some_and(|report| {
                report
                    .articles_reviewed
                    .iter()
                    .chain(report.articles_skipped.iter().map(|s| &s.number))
                    .any(|n| numbers.contains(n))
            });
            if window_newly_enriched == 0 && !report_references_window && !window_new_gaps {
                return Err(PipelineError::Enrich(format!(
                    "{CHUNK_NO_OUTPUT_MARKER}: no new machine_readable additions in the window, \
                     no chunk_report referencing this window, no new markings or untranslatables \
                     in the window (entries {} of {})",
                    numbers.join(", "),
                    articles_before
                )));
            }
        }
        Some(_) => {}
    }

    // Write enrichment metadata. The model is what the CLI said it ran on;
    // only when it named none is it the configured one, `default` when unset.
    let models = session.models();
    let metadata = EnrichmentMetadata {
        law_id: payload.law_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        provider: provider_name.clone(),
        model: if models.is_empty() {
            config.provider.model_str()
        } else {
            models.join(", ")
        },
        prompt_hash: compute_prompt_hash(repo_path).await,
        code_commit: config.code_commit.clone(),
        coverage_score,
        articles_total: articles_before,
        articles_with_machine_readable,
        source_hash: source_hash.to_string(),
        enrich_cursor: next_cursor,
        enrich_cursor_path: normalized_path.clone(),
        enrich_cursor_mode: config.window_mode.label().to_string(),
    };

    let metadata_path = yaml_abs
        .parent()
        .unwrap_or(Path::new("."))
        .join(".enrichment.yaml");
    let metadata_yaml = serde_yaml_ng::to_string(&metadata)
        .map_err(|e| PipelineError::Enrich(format!("failed to serialize metadata: {e}")))?;
    tokio::fs::write(&metadata_path, &metadata_yaml).await?;

    let related_legislation = envelope.related_legislation;

    // Collect written files for corpus staging
    let mut written_files = vec![yaml_abs.clone(), metadata_path];

    // Stage the result envelope as provenance when the agent wrote one.
    let envelope_path = enrichment_result_path(&yaml_abs);
    if envelope_path.exists() {
        written_files.push(envelope_path);
    }

    // Check if a feature file was generated for this specific law.
    // MvT research creates feature files named after the law slug.
    // Only include files whose name contains the law slug to avoid
    // accidentally staging unrelated feature files.
    let law_slug = Path::new(&normalized_path)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());
    let features_dir = repo_path.join("features");
    if let Some(ref slug) = law_slug {
        if features_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&features_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "feature") {
                        if let Some(name) = path.file_stem() {
                            if name.to_string_lossy().contains(slug.as_str()) {
                                written_files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    let branch = enrich_branch_name(&provider_name);

    // What the window cost, and what each call in it cost. One line per call
    // and one for the total: a run is only worth comparing with another if
    // both say which calls were resumed and what each of them took.
    let agent_calls = session.calls();
    let usage = session.total();
    for call in &agent_calls {
        if let Some(u) = call.usage {
            tracing::info!(
                law_id = %payload.law_id,
                step = %call.step,
                resumed = call.resumed,
                input_tokens = u.input_tokens,
                output_tokens = u.output_tokens,
                cache_read_tokens = u.cache_read_tokens,
                cost_millicents = u.cost_millicents,
                "agent call accounted"
            );
        }
    }
    if let Some(u) = usage {
        tracing::info!(
            law_id = %payload.law_id,
            session_reuse = config.session_reuse.label(),
            calls = agent_calls.len(),
            resumed_calls = agent_calls.iter().filter(|c| c.resumed).count(),
            input_tokens = u.input_tokens,
            output_tokens = u.output_tokens,
            cache_read_tokens = u.cache_read_tokens,
            cost_millicents = u.cost_millicents,
            "window accounted"
        );
    }

    let result = EnrichResult {
        law_id: payload.law_id.clone(),
        yaml_path: normalized_path,
        articles_total: articles_before,
        articles_with_machine_readable,
        coverage_score,
        provider: provider_name,
        branch,
        related_legislation,
        untranslatables,
        markings,
        law_complete,
        enrich_cursor: next_cursor,
        feedback,
        usage,
        agent_calls,
        models,
        session_reuse: config.session_reuse.label().to_string(),
    };

    Ok((result, written_files))
}

/// Compute a SHA256 hash of the skill files used in the enrichment prompt.
///
/// This lets you detect when skill instructions changed between enrichments.
async fn compute_prompt_hash(repo_path: &Path) -> String {
    let skill_files = [
        ".claude/skills/law-generate/SKILL.md",
        ".claude/skills/law-generate/reference.md",
        ".claude/skills/law-generate/examples.md",
        ".claude/skills/law-reverse-validate/SKILL.md",
    ];

    let mut hasher = Sha256::new();
    let mut files_found = 0usize;
    for file in &skill_files {
        let path = repo_path.join(file);
        if let Ok(content) = tokio::fs::read(&path).await {
            hasher.update(&content);
            files_found += 1;
        } else {
            tracing::warn!(file = %file, "skill file not found for prompt hash");
        }
    }

    if files_found == 0 {
        tracing::warn!("no skill files found — prompt hash will be empty");
    }

    format!("{:x}", hasher.finalize())
}

/// Count total articles and articles with a `machine_readable` section in one
/// parse pass.
///
/// The law is parsed into the canonical [`ArticleBasedLaw`] model
/// (`regelrecht-law-model`) rather than walked as an untyped YAML value, so the
/// field access is type-checked against the single source of truth for the law
/// format. A structurally-invalid law now surfaces as a parse error here instead
/// of being silently undercounted — acceptable because this only ever runs on
/// real harvested/enriched corpus files, where a corruption is worth failing on.
///
/// An article counts as enriched when it carries a `machine_readable` mapping,
/// including the empty `{}` an LLM may insert before filling it; an explicit
/// `machine_readable: null` is treated as un-enriched. No corpus file uses the
/// bare/null form, so this matches the previous key-presence behavior in practice.
#[cfg(test)]
async fn count_article_stats(path: &Path) -> Result<(usize, usize)> {
    let law = load_law(path).await?;
    Ok(article_stats(&law))
}

/// Parse a law YAML file into the canonical [`ArticleBasedLaw`] model.
async fn load_law(path: &Path) -> Result<ArticleBasedLaw> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(serde_yaml_ng::from_str(&content)?)
}

/// The statutory text of every article, keyed by article number.
///
/// A translator may never edit the law's own words. The article count catches
/// a deleted or added entry and says nothing about the text inside one, so an
/// agent that shortened an article moved no counter at all: the findings did
/// not rise, the schema stayed valid, the count held, and the file was
/// committed with a wrong legal text under a run reporting success. Comparing
/// this map before and after closes that, without needing the network the
/// source gate uses.
fn article_texts(law: &ArticleBasedLaw) -> std::collections::BTreeMap<String, String> {
    law.articles
        .iter()
        .map(|article| (article.number.clone(), article.text.clone()))
        .collect()
}

/// `(articles_total, articles_with_machine_readable)` for a parsed law.
fn article_stats(law: &ArticleBasedLaw) -> (usize, usize) {
    let total = law.articles.len();
    let with_machine_readable = law
        .articles
        .iter()
        .filter(|article| article.machine_readable.is_some())
        .count();
    (total, with_machine_readable)
}

/// Collect all untranslatables from an enriched law YAML, flattened to
/// [`CapturedUntranslatable`] with the owning article number attached.
///
/// Parses the law into the canonical [`ArticleBasedLaw`] model, mirroring
/// [`count_article_stats`]. Returns an empty vec when no article declares any.
#[cfg(test)]
async fn collect_untranslatables(path: &Path) -> Result<Vec<CapturedUntranslatable>> {
    let law = load_law(path).await?;
    Ok(collect_untranslatables_from(&law))
}

/// Window-scoped progress stats for the chunk no-op guard:
/// `(articles_with_machine_readable, recorded_gap_count)` within
/// `[start, end)` of a parsed law. The guard measures progress inside the
/// assigned window only — a whole-document delta would let an edit *outside*
/// the window masquerade as progress for a window that was never reviewed.
///
/// The second figure counts both channels a law may use to record what its
/// model does not do: `markings` from schema v0.7.0 and the `untranslatables`
/// they replaced. Counting only the old one made the guard blind to the
/// commonest legitimate outcome of a window of definition provisions —
/// already-modelled articles that this run only flagged — and failed a window
/// that was reviewed exactly as intended.
fn window_progress_stats(law: &ArticleBasedLaw, numbers: &[String]) -> (usize, usize) {
    let window: Vec<_> = law
        .articles
        .iter()
        .filter(|a| numbers.contains(&a.number))
        .collect();
    let enriched = window
        .iter()
        .filter(|a| a.machine_readable.is_some())
        .count();
    let recorded_gaps = window
        .iter()
        .filter_map(|a| a.machine_readable.as_ref())
        .map(|m| {
            m.untranslatables.as_ref().map_or(0, Vec::len) + m.markings.as_ref().map_or(0, Vec::len)
        })
        .sum();
    (enriched, recorded_gaps)
}

/// Flatten the untranslatables of an already-parsed law. See
/// [`collect_untranslatables`]. The v0.7.0 channel has its own collector,
/// [`collect_markings_from`]: the two are deliberately not merged, because
/// only this one is mirrored into the `untranslatables` table.
fn collect_untranslatables_from(law: &ArticleBasedLaw) -> Vec<CapturedUntranslatable> {
    let mut out = Vec::new();
    for article in &law.articles {
        let Some(machine_readable) = &article.machine_readable else {
            continue;
        };
        let Some(entries) = &machine_readable.untranslatables else {
            continue;
        };
        for entry in entries {
            out.push(CapturedUntranslatable {
                article: article.number.clone(),
                construct: entry.construct.clone(),
                reason: entry.reason.clone(),
                suggestion: entry.suggestion.clone(),
                legal_text_excerpt: entry.legal_text_excerpt.clone(),
                accepted: entry.accepted,
            });
        }
    }
    out
}

/// Flatten the markings of an already-parsed law, with the owning article
/// number attached. The v0.7.0 counterpart of [`collect_untranslatables_from`].
fn collect_markings_from(law: &ArticleBasedLaw) -> Vec<CapturedMarking> {
    let mut out = Vec::new();
    for article in &law.articles {
        let Some(machine_readable) = &article.machine_readable else {
            continue;
        };
        let Some(entries) = &machine_readable.markings else {
            continue;
        };
        for entry in entries {
            out.push(CapturedMarking {
                article: article.number.clone(),
                about: entry.about.clone(),
                resolution: entry.resolution.as_str().to_string(),
                resolved_by: entry.resolved_by.clone(),
                target: entry.target.clone(),
                legal_text_excerpt: entry.legal_text_excerpt.clone(),
                accepted: entry.accepted,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_enrich_base_uses_preferred_when_remote_has_it() {
        assert_eq!(pick_enrich_base("pr574", true), "pr574");
    }

    #[test]
    fn pick_enrich_base_falls_back_when_preferred_missing() {
        // Fresh PR deployment whose harvester hasn't pushed its branch yet:
        // enrichment must fall back to development instead of failing.
        assert_eq!(pick_enrich_base("pr574", false), "development");
    }

    #[test]
    fn pick_enrich_base_short_circuits_for_development() {
        // When the worker's own base is already `development`, the
        // remote-exists bool is moot and we always use `development`.
        assert_eq!(pick_enrich_base("development", true), "development");
        assert_eq!(pick_enrich_base("development", false), "development");
    }

    #[test]
    fn decide_base_action_new_law_checks_out_fresh() {
        assert_eq!(
            decide_base_action(false, None, "sha_new"),
            BaseAction::CheckoutFresh
        );
        // Even if a stored hash somehow exists, an untracked path is a fresh checkout.
        assert_eq!(
            decide_base_action(false, Some("sha_old"), "sha_new"),
            BaseAction::CheckoutFresh
        );
    }

    #[test]
    fn decide_base_action_unchanged_base_skips() {
        assert_eq!(
            decide_base_action(true, Some("sha"), "sha"),
            BaseAction::Skip
        );
    }

    #[test]
    fn decide_base_action_changed_base_is_drift() {
        assert_eq!(
            decide_base_action(true, Some("sha_old"), "sha_new"),
            BaseAction::Drift
        );
    }

    #[test]
    fn decide_base_action_missing_or_empty_provenance_adopts_baseline() {
        // A tracked law with no recorded provenance is a pre-guard "legacy"
        // enrichment: grandfather it by adopting the current base as baseline,
        // never a terminal drift (that would fail every existing enrichment the
        // moment the guard ships).
        assert_eq!(
            decide_base_action(true, None, "sha_new"),
            BaseAction::AdoptBaseline
        );
        assert_eq!(
            decide_base_action(true, Some(""), "sha_new"),
            BaseAction::AdoptBaseline
        );
    }

    #[test]
    fn test_enrich_payload_serde_roundtrip() {
        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: Some("claude".to_string()),
            depth: Some(2),
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: EnrichPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider.as_deref(), Some("claude"));
        assert_eq!(deserialized.depth, Some(2));

        // Verify backward compatibility: provider and depth are optional and
        // skipped when None (old queued payloads omit them entirely).
        let payload_no_provider = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };
        let json_no_provider = serde_json::to_string(&payload_no_provider).unwrap();
        assert!(!json_no_provider.contains("provider"));
        assert!(!json_no_provider.contains("depth"));
        let deserialized_no_provider: EnrichPayload =
            serde_json::from_str(&json_no_provider).unwrap();
        assert!(deserialized_no_provider.provider.is_none());
        assert!(deserialized_no_provider.depth.is_none());

        assert_eq!(deserialized.law_id, "BWBR0018451");
        assert!(deserialized.yaml_path.contains("zorgtoeslag"));
    }

    /// Minimale corpus-brede payload; de guard-tests zetten er traject-velden op.
    fn corpus_wide_payload() -> EnrichPayload {
        EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        }
    }

    #[test]
    fn require_corpus_wide_target_accepts_central_corpus_jobs() {
        // De klassieke corpus-brede enrich (geen traject-velden) blijft
        // gewoon werken met het centrale corpus-token.
        corpus_wide_payload()
            .require_corpus_wide_target()
            .expect("corpus-brede payload hoort door de guard te komen");
    }

    #[test]
    fn require_corpus_wide_target_rejects_traject_payloads() {
        // Worker/traject-contract: het corpus-brede push-pad (centrale repo,
        // centrale token) mag nooit een traject-gerichte payload verwerken —
        // die hoort via de taak-flow af te buigen. traject_id én traject_ref
        // triggeren elk afzonderlijk, zodat een half-gevulde payload (bijv.
        // een nieuwe enqueue die maar één veld zet) niet doorglipt.
        let mut with_id = corpus_wide_payload();
        with_id.traject_id = Some(Uuid::new_v4());
        assert!(with_id.require_corpus_wide_target().is_err());

        let mut with_ref = corpus_wide_payload();
        with_ref.traject_ref = Some("voorbeeld-abcd1234".to_string());
        let err = with_ref.require_corpus_wide_target().unwrap_err();
        assert!(
            err.to_string().contains("review-taak"),
            "de fout moet naar het contract verwijzen, kreeg: {err}"
        );
    }

    #[test]
    fn test_enrich_result_serde() {
        let result = EnrichResult {
            law_id: "BWBR0018451".to_string(),
            yaml_path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml".to_string(),
            articles_total: 10,
            articles_with_machine_readable: 7,
            coverage_score: 0.7,
            provider: "opencode".to_string(),
            branch: "enrich/opencode".to_string(),
            related_legislation: Vec::new(),
            untranslatables: Vec::new(),
            markings: Vec::new(),
            law_complete: true,
            enrich_cursor: 0,
            feedback: Vec::new(),
            usage: None,
            agent_calls: Vec::new(),
            models: Vec::new(),
            session_reuse: SessionReuse::Off.label().to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["articles_with_machine_readable"], 7);
        assert_eq!(json["coverage_score"], 0.7);
        assert_eq!(json["provider"], "opencode");
        assert_eq!(json["branch"], "enrich/opencode");
        assert_eq!(json["law_complete"], true);
        assert_eq!(json["enrich_cursor"], 0);
    }

    #[test]
    fn enrich_result_law_complete_defaults_true_for_legacy_json() {
        // `jobs.result` rows written before chunking existed lack both fields;
        // they always covered the whole law, so they must deserialize as
        // complete with cursor 0.
        let legacy = serde_json::json!({
            "law_id": "BWBR0018451",
            "yaml_path": "regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
            "articles_total": 10,
            "articles_with_machine_readable": 7,
            "coverage_score": 0.7,
            "provider": "opencode",
            "branch": "enrich/opencode",
        });
        let result: EnrichResult = serde_json::from_value(legacy).unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 0);
    }

    #[test]
    fn test_envelope_full_deserialization() {
        let yaml = r#"
law_id: wet_op_de_zorgtoeslag
related_legislation:
  - name: Regeling vaststelling standaardpremie en bestuursrechtelijke premie
    relation: delegated_regeling
    bwb_id: BWBR0037841
    slug: regeling_standaardpremie
    open_term: standaardpremie
  - name: Algemene wet inkomensafhankelijke regelingen
    relation: source_regulation
"#;
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(envelope.law_id.as_deref(), Some("wet_op_de_zorgtoeslag"));
        assert_eq!(envelope.related_legislation.len(), 2);
        let first = &envelope.related_legislation[0];
        assert_eq!(first.relation, "delegated_regeling");
        assert_eq!(first.bwb_id.as_deref(), Some("BWBR0037841"));
        assert_eq!(first.slug.as_deref(), Some("regeling_standaardpremie"));
        assert_eq!(first.open_term.as_deref(), Some("standaardpremie"));
        // Second entry omits every optional field.
        let second = &envelope.related_legislation[1];
        assert_eq!(second.relation, "source_regulation");
        assert!(second.bwb_id.is_none());
        assert!(second.slug.is_none());
        assert!(second.open_term.is_none());
    }

    #[test]
    fn test_envelope_missing_fields_default() {
        // Only `name` is required; everything else defaults.
        let yaml = "related_legislation:\n  - name: Some Law\n";
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(envelope.law_id.is_none());
        assert_eq!(envelope.related_legislation.len(), 1);
        let entry = &envelope.related_legislation[0];
        assert_eq!(entry.name, "Some Law");
        assert_eq!(entry.relation, "");
        assert!(entry.bwb_id.is_none());
    }

    #[tokio::test]
    async fn test_read_envelope_absent_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        // No sidecar exists next to it.
        let envelope = read_enrichment_result_envelope(&yaml_abs).await;
        assert!(envelope.related_legislation.is_empty());
        assert!(envelope.chunk_report.is_none());
    }

    #[tokio::test]
    async fn test_read_envelope_malformed_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        std::fs::write(
            enrichment_result_path(&yaml_abs),
            "related_legislation: [this is: not valid: yaml",
        )
        .unwrap();
        // Malformed sidecar must never error — it degrades to empty.
        let envelope = read_enrichment_result_envelope(&yaml_abs).await;
        assert!(envelope.related_legislation.is_empty());
        assert!(envelope.chunk_report.is_none());
    }

    #[tokio::test]
    async fn test_read_envelope_present_parses() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_abs = dir.path().join("2025-01-01.yaml");
        std::fs::write(
            enrichment_result_path(&yaml_abs),
            "related_legislation:\n  - name: Delegated Regeling\n    bwb_id: BWBR0037841\n",
        )
        .unwrap();
        let related = read_enrichment_result_envelope(&yaml_abs)
            .await
            .related_legislation;
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].bwb_id.as_deref(), Some("BWBR0037841"));
    }

    #[test]
    fn test_llm_provider_opencode_defaults() {
        let provider = LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        };
        assert_eq!(provider.name(), "opencode");
        assert_eq!(provider.model_str(), "default");
    }

    #[test]
    fn test_llm_provider_claude_with_model() {
        let provider = LlmProvider::Claude {
            path: "/usr/local/bin/claude".into(),
            model: Some("opus".into()),
        };
        assert_eq!(provider.name(), "claude");
        assert_eq!(provider.model_str(), "opus");
    }

    fn test_config(provider: LlmProvider) -> EnrichConfig {
        EnrichConfig::for_test(provider)
    }

    #[test]
    fn test_with_provider_override() {
        let base_config = test_config(LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        });

        let claude_config = base_config.with_provider_override("claude");
        assert_eq!(claude_config.provider.name(), "claude");
        assert_eq!(claude_config.timeout, Duration::from_secs(600));
        assert_eq!(claude_config.code_commit, "abc123");
        // The memory ceiling must survive a provider override.
        assert_eq!(claude_config.max_rss_mb, 3500);

        let opencode_config = base_config.with_provider_override("opencode");
        assert_eq!(opencode_config.provider.name(), "opencode");

        // Unknown provider falls back to current provider
        let unknown_config = base_config.with_provider_override("unknown");
        assert_eq!(unknown_config.provider.name(), "opencode");
    }

    /// The enqueue path always names a provider, so this override runs on
    /// every production job. Hardcoding `RunSteps::all()` here made
    /// `ENRICH_STEPS` dead in the worker: whatever `from_env` parsed was
    /// discarded before the run began.
    #[test]
    fn a_provider_override_keeps_the_steps_the_run_was_given() {
        let mut base_config = test_config(LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        });
        base_config.steps = RunSteps {
            window: false,
            reconcile: true,
        };
        let overridden = base_config.with_provider_override("claude");
        assert_eq!(
            overridden.steps, base_config.steps,
            "the steps must survive a provider override"
        );
    }

    /// A schema error names its article through the instance path, and a
    /// windowed run may only be asked about its own articles. Handing an
    /// agent an error about article 40 while it holds a window of [1, 2] is
    /// an instruction to break the rule the chunking rests on, and refusing
    /// hard-fails the job: a pre-existing defect elsewhere in the law would
    /// block every window of that law forever.
    #[test]
    fn a_schema_error_outside_the_window_is_not_handed_back() {
        let raw = r#"
articles:
  - number: '1'
    text: eerste
  - number: '40'
    text: veertigste
"#;
        assert_eq!(
            schema_error_article(raw, "/articles/1/machine_readable: iets mis"),
            Some("40".to_string())
        );
        assert_eq!(
            schema_error_article(raw, "/articles/0/machine_readable: iets mis"),
            Some("1".to_string())
        );
        // An error about the document as a whole belongs to every window.
        assert_eq!(
            schema_error_article(raw, "$schema: missing or unknown schema version"),
            None
        );
        let window = vec!["1".to_string()];
        assert!(in_window(Some(&window), Some("1")));
        assert!(!in_window(Some(&window), Some("40")));
        assert!(in_window(Some(&window), None));
    }

    #[test]
    fn test_enrich_providers_list() {
        assert!(ENRICH_PROVIDERS.contains(&"opencode"));
        assert!(ENRICH_PROVIDERS.contains(&"claude"));
        assert_eq!(ENRICH_PROVIDERS.len(), 2);
    }

    #[test]
    fn test_select_claude_token() {
        // empty / whitespace-only -> None
        assert_eq!(select_claude_token("", 0), None);
        assert_eq!(select_claude_token("  , ,", 5), None);

        // single token -> always that token, index 0
        assert_eq!(select_claude_token("tokA", 0), Some((0, 1, "tokA")));
        assert_eq!(select_claude_token(" tokA ", 999), Some((0, 1, "tokA")));

        // multiple tokens -> rotate by bucket % n, whitespace trimmed
        assert_eq!(select_claude_token("a, b , c", 0), Some((0, 3, "a")));
        assert_eq!(select_claude_token("a, b , c", 1), Some((1, 3, "b")));
        assert_eq!(select_claude_token("a, b , c", 2), Some((2, 3, "c")));
        assert_eq!(select_claude_token("a, b , c", 3), Some((0, 3, "a")));
        // large bucket (e.g. unix_secs/100) still wraps correctly
        assert_eq!(select_claude_token("a,b", 17_000_001), Some((1, 2, "b")));
    }

    #[test]
    fn test_parse_vmrss_kb_extracts_value() {
        let status = "Name:\tnode\nVmPeak:\t 4194304 kB\nVmRSS:\t  2097152 kB\nThreads:\t12\n";
        assert_eq!(parse_vmrss_kb(status), Some(2_097_152));
    }

    #[test]
    fn test_parse_vmrss_kb_missing_or_malformed() {
        // No VmRSS line.
        assert_eq!(parse_vmrss_kb("Name:\tnode\nThreads:\t12\n"), None);
        // VmRSS present but value not numeric.
        assert_eq!(parse_vmrss_kb("VmRSS:\t  notanumber kB\n"), None);
        // Empty input.
        assert_eq!(parse_vmrss_kb(""), None);
    }

    #[test]
    fn test_enrich_config_default_timeout() {
        let config = test_config(LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        });
        assert_eq!(config.timeout, Duration::from_secs(600));
        assert_eq!(config.provider.name(), "opencode");
    }

    #[test]
    fn test_build_prompt_contains_skill_paths() {
        let prompt = build_prompt(
            "regulation/nl/wet/test/2025-01-01.yaml",
            "/tmp/repo/regulation/nl/wet/test/.enrichment-progress.json",
            &full_plan(),
            None,
            false,
            false,
        );
        assert!(prompt.contains("law-generate/SKILL.md"));
        assert!(prompt.contains("law-reverse-validate/SKILL.md"));
        assert!(prompt.contains("regulation/nl/wet/test/2025-01-01.yaml"));
        assert!(prompt.contains(".enrichment-progress.json"));
    }

    #[test]
    fn test_enrich_branch_name() {
        assert_eq!(enrich_branch_name("opencode"), "enrich/opencode");
        assert_eq!(enrich_branch_name("claude"), "enrich/claude");
    }

    #[test]
    fn test_enrich_payload_task_fields_roundtrip_and_backcompat() {
        // Oude payloads (zonder taak-velden) moeten blijven deserialiseren.
        let old = serde_json::json!({"law_id": "x", "yaml_path": "nl/x/2025-01-01.yaml"});
        let parsed: EnrichPayload = serde_json::from_value(old).unwrap();
        assert!(parsed.requested_by.is_none());
        assert!(!parsed.deliver_as_task());

        // Nieuwe payloads dragen de taak-velden mee.
        let account = uuid::Uuid::new_v4();
        let new = EnrichPayload {
            pass: Pass::Translate,
            law_id: "x".into(),
            yaml_path: "laws/x/law.yaml".into(),
            provider: Some("claude".into()),
            depth: None,
            requested_by: Some(account),
            deliver: Some("task".into()),
            traject_id: Some(uuid::Uuid::new_v4()),
            traject_ref: Some("testtraject-abcd1234".into()),
            source_etag: Some("\"abc\"".into()),
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };
        let roundtrip: EnrichPayload =
            serde_json::from_value(serde_json::to_value(&new).unwrap()).unwrap();
        assert_eq!(roundtrip.requested_by, Some(account));
        assert!(roundtrip.deliver_as_task());
    }

    #[test]
    fn test_enrichment_metadata_serde() {
        let meta = EnrichmentMetadata {
            law_id: "BWBR0018451".to_string(),
            timestamp: "2026-03-12T10:00:00Z".to_string(),
            provider: "opencode".to_string(),
            model: "vlam/mistral-medium".to_string(),
            prompt_hash: "abc123".to_string(),
            code_commit: "deadbeef".to_string(),
            coverage_score: 0.7,
            articles_total: 10,
            articles_with_machine_readable: 7,
            source_hash: String::new(),
            enrich_cursor: 0,
            enrich_cursor_path: String::new(),
            enrich_cursor_mode: String::new(),
        };

        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        assert!(yaml.contains("law_id: BWBR0018451"));
        assert!(yaml.contains("provider: opencode"));

        let deserialized: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(deserialized.articles_with_machine_readable, 7);
    }

    #[test]
    fn enrichment_metadata_source_hash_defaults_when_absent() {
        // A .enrichment.yaml written before this field existed.
        let legacy = "law_id: BWBR0001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: claude\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 1\narticles_with_machine_readable: 1\n";
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(legacy).unwrap();
        assert_eq!(meta.source_hash, "");
        // Cursor fields default too (files written before chunking existed).
        assert_eq!(meta.enrich_cursor, 0);
        assert_eq!(meta.enrich_cursor_path, "");
    }

    #[test]
    fn enrichment_metadata_source_hash_roundtrips() {
        let meta = EnrichmentMetadata {
            law_id: "BWBR0001".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            provider: "claude".into(),
            model: "m".into(),
            prompt_hash: "p".into(),
            code_commit: "c".into(),
            coverage_score: 1.0,
            articles_total: 1,
            articles_with_machine_readable: 1,
            source_hash: "abc123".into(),
            enrich_cursor: 30,
            enrich_cursor_path: "regulation/nl/wet/x/2026-01-01.yaml".into(),
            enrich_cursor_mode: "document".into(),
        };
        let yaml = serde_yaml_ng::to_string(&meta).unwrap();
        let back: EnrichmentMetadata = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(back.source_hash, "abc123");
        assert_eq!(back.enrich_cursor, 30);
        assert_eq!(
            back.enrich_cursor_path,
            "regulation/nl/wet/x/2026-01-01.yaml"
        );
    }

    #[test]
    fn test_normalize_yaml_path_valid() {
        assert_eq!(
            normalize_yaml_path("regulation/nl/wet/zorgtoeslag/2025-01-01.yaml").unwrap(),
            "regulation/nl/wet/zorgtoeslag/2025-01-01.yaml"
        );
        assert_eq!(
            normalize_yaml_path("regulation/nl/ministeriele_regeling/test/file.yaml").unwrap(),
            "regulation/nl/ministeriele_regeling/test/file.yaml"
        );
    }

    #[test]
    fn test_normalize_yaml_path_strips_known_prefixes() {
        assert_eq!(
            normalize_yaml_path("/tmp/corpus-repo/regulation/nl/wet/test/2025-01-01.yaml").unwrap(),
            "regulation/nl/wet/test/2025-01-01.yaml"
        );
        assert_eq!(
            normalize_yaml_path("/tmp/regulation-repo/regulation/nl/wet/test/2025-01-01.yaml")
                .unwrap(),
            "regulation/nl/wet/test/2025-01-01.yaml"
        );
    }

    #[test]
    fn test_normalize_yaml_path_rejects_unknown_absolute() {
        assert!(normalize_yaml_path("/etc/passwd").is_err());
        assert!(normalize_yaml_path("/other/path/file.yaml").is_err());
    }

    #[test]
    fn test_normalize_yaml_path_rejects_traversal() {
        assert!(normalize_yaml_path("../etc/passwd").is_err());
        assert!(normalize_yaml_path("regulation/../../etc/passwd").is_err());
    }

    #[test]
    fn test_normalize_yaml_path_rejects_special_chars() {
        assert!(normalize_yaml_path("regulation/nl/wet/test; rm -rf /").is_err());
        assert!(normalize_yaml_path("regulation/nl/wet/test$(whoami)").is_err());
        assert!(normalize_yaml_path("").is_err());
    }

    #[tokio::test]
    async fn test_count_article_stats() {
        // A realistic minimal article-based law: typed counting requires the
        // canonical top-level fields ($id/regulatory_layer/publication_date) and
        // articles with number+text, so the fixture mirrors a real harvested law.
        let yaml = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
  - number: '2'
    text: Article two.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
  - number: '3'
    text: Article three.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel3
    machine_readable:
      execution:
        actions: []
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let (total, with_mr) = count_article_stats(&path).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(with_mr, 1);
    }

    #[tokio::test]
    async fn test_collect_untranslatables() {
        // Two articles carry untranslatables (one accepted, one not); a third
        // article has a machine_readable section without any. The collector must
        // flatten every entry, attach the owning article number, and preserve the
        // optional fields + accepted flag.
        let yaml = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
    machine_readable:
      untranslatables:
        - construct: rounding
          reason: Engine cannot round yet.
          suggestion: Add a ROUND operation.
          legal_text_excerpt: naar boven afgerond op hele euro's
          accepted: false
  - number: '2'
    text: Article two.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
    machine_readable:
      execution:
        actions: []
  - number: '3'
    text: Article three.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel3
    machine_readable:
      untranslatables:
        - construct: table_lookup
          reason: Table lookup unsupported.
          accepted: true
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let collected = collect_untranslatables(&path).await.unwrap();
        assert_eq!(collected.len(), 2);

        let rounding = collected
            .iter()
            .find(|u| u.construct == "rounding")
            .expect("rounding entry");
        assert_eq!(rounding.article, "1");
        assert_eq!(rounding.reason, "Engine cannot round yet.");
        assert_eq!(
            rounding.suggestion.as_deref(),
            Some("Add a ROUND operation.")
        );
        assert_eq!(
            rounding.legal_text_excerpt.as_deref(),
            Some("naar boven afgerond op hele euro's")
        );
        assert!(!rounding.accepted);

        let lookup = collected
            .iter()
            .find(|u| u.construct == "table_lookup")
            .expect("table_lookup entry");
        assert_eq!(lookup.article, "3");
        assert!(lookup.suggestion.is_none());
        assert!(lookup.legal_text_excerpt.is_none());
        assert!(lookup.accepted);
    }

    #[tokio::test]
    async fn test_collect_untranslatables_none() {
        // A law that records its gap the v0.7.0 way: the untranslatables
        // collector must find nothing and the marking collector everything.
        // The two channels are separate on purpose — the one the worker
        // mirrors into the v1 table may not silently absorb the other.
        let yaml = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
    machine_readable:
      markings:
        - about: fixture
          reason: het formaat kent hier geen vorm voor deze constructie
          resolution: model
          target: []
          legal_text_excerpt: Article one.
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        assert!(collect_untranslatables(&path).await.unwrap().is_empty());

        let law = load_law(&path).await.unwrap();
        let markings = collect_markings_from(&law);
        assert_eq!(markings.len(), 1);
        assert_eq!(markings[0].article, "1");
        assert_eq!(markings[0].about, "fixture");
        assert_eq!(markings[0].resolution, "model");
        assert_eq!(markings[0].legal_text_excerpt, "Article one.");
        assert!(!markings[0].accepted);
        assert!(markings[0].resolved_by.is_none());
    }

    // ---- the closing pass -----------------------------------------------

    /// A law in the state a chunked walk leaves behind: entry 1 reads
    /// `standaardpremie` as a bare input because entry 2, which produces it,
    /// had no model yet when entry 1 was written.
    pub(super) const TOO_EARLY_LAW: &str = r"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De hoogte is gelijk aan de standaardpremie, bedoeld in artikel 2.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
    machine_readable:
      endpoint: hoogte
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
        parameters:
          - name: bsn
            type: string
            required: true
            description: Het burgerservicenummer van de verzekerde.
        input:
          - name: standaardpremie
            type: amount
            description: De standaardpremie voor het berekeningsjaar.
        output:
          - name: hoogte
            type: amount
            description: De hoogte van de tegemoetkoming.
        actions:
          - output: hoogte
            value: $standaardpremie
            legal_basis:
              law: Testwet
              bwb_id: BWBR0000001
              article: '1'
              explanation: Het artikel stelt de hoogte gelijk aan de standaardpremie.
  - number: '2'
    text: De standaardpremie wordt jaarlijks vastgesteld.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
    machine_readable:
      endpoint: standaardpremie
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
        parameters:
          - name: bsn
            type: string
            required: true
            description: Het burgerservicenummer van de verzekerde.
        output:
          - name: standaardpremie
            type: amount
            description: De jaarlijks vastgestelde standaardpremie.
        actions:
          - output: standaardpremie
            value: 1000
            legal_basis:
              law: Testwet
              bwb_id: BWBR0000001
              article: '2'
              explanation: Het artikel stelt de standaardpremie jaarlijks vast.
";

    /// The whole point: the binding entry 1 could not lay is laid by the
    /// worker, without an agent and without anything else moving.
    #[tokio::test]
    async fn the_closing_pass_binds_what_the_window_could_not_see() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        tokio::fs::write(&path, TOO_EARLY_LAW).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        let runner = NoopLlmRunner;
        let progress = run_closing_reconcile(&path, dir.path(), &payload, &config, &runner)
            .await
            .unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&after).unwrap();
        let source = &doc["articles"][0]["machine_readable"]["execution"]["input"][0]["source"];
        assert_eq!(source["output"].as_str(), Some("standaardpremie"));
        assert_eq!(source["parameters"]["bsn"].as_str(), Some("$bsn"));
        // Nothing else moved: the diff is the four inserted lines.
        assert_eq!(
            after.lines().count(),
            TOO_EARLY_LAW.lines().count() + 4,
            "de afrondende pass schrijft alleen het source-blok"
        );
        // No lead left, so no agent was asked.
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].gate, "reconcile");
        assert_eq!(progress[0].findings_initial, 0);
        assert!(progress[0].rounds.is_empty());
    }

    /// A law with nothing to connect is left byte-identical. The pass runs on
    /// every completed law, so "does nothing" has to mean nothing at all.
    #[tokio::test]
    async fn the_closing_pass_leaves_a_connected_law_alone() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        // Run it once to connect, then again over the result.
        tokio::fs::write(&path, TOO_EARLY_LAW).await.unwrap();
        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        let runner = NoopLlmRunner;
        run_closing_reconcile(&path, dir.path(), &payload, &config, &runner)
            .await
            .unwrap();
        let once = tokio::fs::read_to_string(&path).await.unwrap();
        run_closing_reconcile(&path, dir.path(), &payload, &config, &runner)
            .await
            .unwrap();
        let twice = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(once, twice, "de pass is idempotent");
    }

    /// The guard, not the intent: a pass that does not lower the finding count
    /// puts the file back. Modelled by handing it a producer whose output is
    /// declared twice — the plan then holds a lead and no link, so nothing is
    /// written and the file stands.
    #[tokio::test]
    async fn the_closing_pass_refuses_to_guess_between_two_producers() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        // Two entries producing `standaardpremie`: entry 2 and a copy as 3.
        let second = TOO_EARLY_LAW
            .split("  - number: '2'\n")
            .nth(1)
            .unwrap()
            .to_string();
        let law = format!(
            "{TOO_EARLY_LAW}  - number: '3'\n{}",
            second
                .replace("article: '2'", "article: '3'")
                .replace("#Artikel2", "#Artikel3")
        );
        tokio::fs::write(&path, &law).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        let runner = NoopLlmRunner;
        let progress = run_closing_reconcile(&path, dir.path(), &payload, &config, &runner)
            .await
            .unwrap();
        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(after, law, "bij twijfel blijft het bestand zoals het stond");
        assert_eq!(progress[0].findings_initial, 1, "de agent krijgt de vraag");
    }

    /// The threshold of the deterministic half, on the branch the planner
    /// cannot be talked into producing.
    ///
    /// The refusing branch had no coverage at all: the test named after it
    /// hands the pass a plan without links, so `if !plan.links.is_empty()`
    /// is false and the whole counting block is skipped. This measures the
    /// decision itself, over two real laws.
    #[test]
    fn de_deterministische_helft_schrijft_niet_als_er_niets_verbetert() {
        let root = tempfile::tempdir().unwrap();
        // The same law twice: nothing got worse and nothing got better, which
        // is the case the pass used to write on.
        let equal = Rewrite::measure(TOO_EARLY_LAW, TOO_EARLY_LAW, root.path());
        assert_eq!(equal.findings_before, equal.findings_after);
        assert_eq!(equal.schema_before, equal.schema_after);
        assert!(
            !equal.is_progress(),
            "gelijk is geen vooruitgang: een verdwenen plus een nieuwe \
             bevinding telt ook gelijk"
        );
        assert!(
            equal.is_not_worse(),
            "voor de agent-helft is gelijk wél goed genoeg"
        );

        // One finding traded for another: equal on both counts, so refused,
        // and this is the case a count alone cannot tell from a clean run.
        let swap = Rewrite {
            findings_before: 3,
            findings_after: 3,
            schema_before: 0,
            schema_after: 0,
            readable: true,
        };
        assert!(!swap.is_progress());

        // Both accepting sides, so the guard is not simply closed: a finding
        // fewer, or a schema error repaired while the findings stand still —
        // which is what binding an input without `source` actually does.
        assert!(Rewrite {
            findings_after: 2,
            ..swap
        }
        .is_progress());
        assert!(Rewrite {
            schema_before: 1,
            ..swap
        }
        .is_progress());
    }

    /// Whatever the counts say, a law the parser cannot read never replaces
    /// the one that stood — and it may not pass as an improvement either,
    /// which it would if unreadability were read off the schema count.
    #[test]
    fn geen_van_beide_helften_schrijft_een_onleesbare_wet_weg() {
        let base = Rewrite {
            findings_before: 5,
            findings_after: 0,
            schema_before: 3,
            schema_after: 0,
            readable: true,
        };
        assert!(base.is_progress() && base.is_not_worse());
        let broken_schema = Rewrite {
            schema_before: 0,
            schema_after: 1,
            ..base
        };
        assert!(!broken_schema.is_progress() && !broken_schema.is_not_worse());
        let unreadable = Rewrite {
            readable: false,
            schema_after: 1,
            ..base
        };
        assert!(!unreadable.is_progress() && !unreadable.is_not_worse());
    }

    /// Answers every feedback round by writing a marking without
    /// `resolved_by`: schema v0.7.0 requires the field and the law model does
    /// not, so this is exactly the edit that used to reach the commit.
    struct SchemaBreakingRunner;

    #[async_trait::async_trait]
    impl LlmRunner for SchemaBreakingRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let text = tokio::fs::read_to_string(yaml_abs).await?;
            let broken = text.replace(
                "    machine_readable:\n      endpoint: hoogte\n",
                "    machine_readable:\n      endpoint: hoogte\n      markings:\n        \
                 - about: de standaardpremie\n          reason: het model kent hier geen vorm voor\n          \
                 resolution: model\n          target: []\n          legal_text_excerpt: de standaardpremie\n",
            );
            assert_ne!(broken, text, "de testrunner moet iets veranderen");
            tokio::fs::write(yaml_abs, broken).await?;
            Ok(())
        }
    }

    /// The agent half had no rollback at all: `run_feedback_rounds` stops and
    /// leaves standing whatever was written. This is the last writer of the
    /// run, so what it leaves is what gets committed.
    #[tokio::test]
    async fn de_afrondende_pass_draait_een_agent_terug_die_het_schema_breekt() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        // Two producers of `standaardpremie`, so the plan holds a lead, no
        // link, and the agent is asked.
        let second = TOO_EARLY_LAW
            .split("  - number: '2'\n")
            .nth(1)
            .unwrap()
            .to_string();
        let law = format!(
            "{TOO_EARLY_LAW}  - number: '3'\n{}",
            second
                .replace("article: '2'", "article: '3'")
                .replace("#Artikel2", "#Artikel3")
        );
        tokio::fs::write(&path, &law).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        run_closing_reconcile(&path, dir.path(), &payload, &config, &SchemaBreakingRunner)
            .await
            .unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(
            after, law,
            "de agent-helft van de afrondende pass moet terugdraaien wat zij verslechtert"
        );
        assert!(
            !after.contains("markings:"),
            "de markering zonder resolved_by — die het schema afwijst en het \
             model accepteert — staat niet in het bestand dat gecommit wordt"
        );
    }

    /// Answers a round with an edit that costs nothing: a comment line.
    struct HarmlessRunner;

    #[async_trait::async_trait]
    impl LlmRunner for HarmlessRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let text = tokio::fs::read_to_string(yaml_abs).await?;
            tokio::fs::write(yaml_abs, format!("{text}# nagelezen\n")).await?;
            Ok(())
        }
    }

    /// The other direction of the same guard: an edit that leaves the counts
    /// where they were stands. A rollback that fires on everything would
    /// silently turn the agent half off.
    #[tokio::test]
    async fn de_afrondende_pass_laat_een_agent_staan_die_niets_verslechtert() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        let second = TOO_EARLY_LAW
            .split("  - number: '2'\n")
            .nth(1)
            .unwrap()
            .to_string();
        let law = format!(
            "{TOO_EARLY_LAW}  - number: '3'\n{}",
            second
                .replace("article: '2'", "article: '3'")
                .replace("#Artikel2", "#Artikel3")
        );
        tokio::fs::write(&path, &law).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        run_closing_reconcile(&path, dir.path(), &payload, &config, &HarmlessRunner)
            .await
            .unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(
            after.contains("# nagelezen"),
            "wat niets verslechtert blijft staan"
        );
    }

    /// A file the agent made unreadable used to report itself as a clean gate:
    /// `Gate::Reconcile` was the one arm of the match that swallowed the parse
    /// error, and an empty finding list is what the round loop reads as
    /// "nothing left to do".
    #[tokio::test]
    async fn de_reconcile_poort_meldt_een_onleesbaar_bestand_in_plaats_van_te_zwijgen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2025-01-01.yaml");
        tokio::fs::write(
            &path,
            "articles:\n  - number: '1'\n    explanation: Eerste lid: de wet.\n",
        )
        .await
        .unwrap();

        let reading = evaluate_gate(Gate::Reconcile, &path, dir.path(), None)
            .await
            .unwrap();
        assert_eq!(reading.answerable.len(), 1, "{:?}", reading.answerable);
        assert!(
            reading.answerable[0].contains("YAML parse error"),
            "de poort noemt de parsefout: {}",
            reading.answerable[0]
        );
    }

    // ---- feedback rounds ------------------------------------------------

    /// A law whose articles carry text and nothing else. Every article is one
    /// `accounted` finding at the marking gate, so the gate's finding count
    /// is the number of articles still passed over in silence.
    fn silent_law(articles: usize) -> String {
        let mut yaml = String::from(
            "---\n\
             $schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json\n\
             $id: test_law\n\
             regulatory_layer: WET\n\
             publication_date: '2025-01-01'\n\
             bwb_id: BWBR0000001\n\
             url: https://wetten.overheid.nl/BWBR0000001/2025-01-01\n\
             valid_from: '2025-01-01'\n\
             articles:\n",
        );
        for n in 1..=articles {
            yaml.push_str(&format!(
                "  - number: '{n}'\n    text: De raad stelt de vergoeding vast naar redelijkheid en billijkheid.\n    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel{n}\n"
            ));
        }
        yaml
    }

    /// Answers a feedback round by filing one marking on the first article
    /// that has none: the finding count falls by one and the marking count
    /// rises by one, which is exactly the trade the measurement has to make
    /// visible.
    struct MarkingRunner {
        calls: std::sync::Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl LlmRunner for MarkingRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            #[allow(clippy::unwrap_used)]
            {
                *self.calls.lock().unwrap() += 1;
            }
            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut article_map) = article {
                            if article_map.contains_key("machine_readable") {
                                continue;
                            }
                            let marking: serde_yaml_ng::Value = serde_yaml_ng::from_str(
                                "markings:\n  - about: naar redelijkheid en billijkheid is een open norm\n    reason: het model kent alleen formules en geen oordeelsruimte\n    resolution: model\n    resolved_by: het formaat zou een oordeelsruimte moeten kunnen dragen die geen formule is\n    target: []\n    legal_text_excerpt: naar redelijkheid en billijkheid\n",
                            )?;
                            article_map.insert(
                                serde_yaml_ng::Value::String("machine_readable".into()),
                                marking,
                            );
                            break;
                        }
                    }
                }
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&value)?).await?;
            Ok(())
        }
    }

    /// Sets the law up on disk and returns `(dir, absolute yaml path)`.
    async fn silent_law_on_disk(articles: usize) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        tokio::fs::write(&path, silent_law(articles)).await.unwrap();
        (dir, path)
    }

    fn rounds_config(rounds: FeedbackRounds) -> EnrichConfig {
        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.feedback_rounds = rounds;
        config
    }

    #[test]
    fn a_finding_outside_the_window_is_not_this_runs_question() {
        // The contradiction round 5 ran into: the prompt says leave every
        // article outside your list completely untouched, and the gate handed
        // the same agent findings about articles 3 through 8.
        let window = vec!["1".to_owned(), "2".to_owned()];
        assert!(in_window(Some(&window), Some("1")));
        assert!(!in_window(Some(&window), Some("3")));

        // Granularity does not decide ownership, in either direction.
        assert!(in_window(Some(&window), Some("2.1.a")));
        assert!(in_window(Some(&["3.2".to_owned()]), Some("3")));

        // A whole-law run owns everything, and a finding about the file rather
        // than about an article is nobody's to duck.
        assert!(in_window(None, Some("9")));
        assert!(in_window(Some(&window), None));
    }

    #[tokio::test]
    async fn a_finding_no_edit_here_can_answer_is_recorded_and_never_asked() {
        // Twelve of the sixteen findings that survived round 5's checks gate
        // were of the form `"zorgverzekeringswet" does not produce output
        // "is_verzekerde"` against a corpus of laws that carry no model at
        // all. No edit to this file makes another law produce an output, so
        // every round could only return the same list. They are still work,
        // so they land in the record instead of in a round.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        tokio::fs::write(
            &path,
            r#"$id: test_law
name: Testwet
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De aanspraak bestaat voor de verzekerde.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        input:
          - name: is_verzekerde
            type: boolean
            source:
              regulation: wet_die_niet_bestaat
              output: is_verzekerde
        actions:
          - output: aanspraak
            value: $is_verzekerde
"#,
        )
        .await
        .unwrap();

        let reading = evaluate_gate(Gate::Checks, &path, dir.path(), None)
            .await
            .unwrap();
        assert_eq!(reading.outside_corpus.len(), 1, "{reading:?}");
        assert!(
            reading.outside_corpus[0].contains("wet_die_niet_bestaat"),
            "{reading:?}"
        );
        assert!(
            !reading
                .answerable
                .iter()
                .any(|f| f.contains("wet_die_niet_bestaat")),
            "an unanswerable finding reached the loop: {reading:?}"
        );

        // And a run over it puts none of them to the agent while keeping them
        // in the record.
        let config = rounds_config(FeedbackRounds {
            checks: 2,
            ..FeedbackRounds::default()
        });
        let runner = MarkingRunner {
            calls: std::sync::Mutex::new(0),
        };
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");
        let progress = run_feedback_rounds(
            Gate::Checks,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();
        assert_eq!(progress.outside_corpus.len(), 1);
        assert_eq!(
            *runner.calls.lock().unwrap(),
            0,
            "an agent was called for a finding it cannot answer"
        );
    }

    #[tokio::test]
    async fn markings_are_counted_even_when_the_law_model_cannot_read_the_file() {
        // The regression, from the run that lost the count. Two shapes broke
        // it there: `requires` written as a list of mappings against a model
        // that had a list of strings, and an intra-law `overrides` entry with
        // no `law` key. Every marking count in that run came back `None`, so
        // the one number that would have shown round 3 buying a lower finding
        // count with more markings showed nothing. The second shape is the one
        // pinned here because the model still rejects it, and the assertion
        // below fails loudly if that ever stops being true.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2026-01-01.yaml");
        tokio::fs::write(
            &path,
            r#"$id: test_law
articles:
  - number: '1'
    text: tekst
    machine_readable:
      overrides:
        - article: '4'
          output: hoogte
          voids: true
      markings:
        - about: iets
          reason: waarom
          resolution: operation
          target: []
          legal_text_excerpt: woorden
  - number: '2'
    text: tekst
    machine_readable:
      markings:
        - about: iets anders
          reason: waarom
          resolution: model
          target: []
          legal_text_excerpt: woorden
"#,
        )
        .await
        .unwrap();

        assert!(
            load_law(&path).await.is_err(),
            "fixture must be a file the law model rejects, or it proves nothing"
        );
        assert_eq!(marking_count(&path).await, Some(2));

        // And `None` still means what it says: unreadable, not "zero".
        tokio::fs::write(&path, "articles: [ this is not yaml\n")
            .await
            .unwrap();
        assert_eq!(marking_count(&path).await, None);
    }

    #[tokio::test]
    async fn a_second_round_runs_and_is_measured_per_round() {
        // Three articles, three findings, two rounds allowed. Each round
        // answers one article, so round 1 takes one finding away and round 2
        // takes the next — the thing the whole exercise is meant to measure.
        let (dir, path) = silent_law_on_disk(3).await;
        let config = rounds_config(FeedbackRounds {
            marking: 2,
            ..FeedbackRounds::default()
        });
        let runner = MarkingRunner {
            calls: std::sync::Mutex::new(0),
        };
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");

        let progress = run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();

        assert_eq!(progress.gate, "marking");
        assert_eq!(progress.findings_initial, 3);
        assert_eq!(progress.findings_final, 1);
        assert_eq!(*runner.calls.lock().unwrap(), 2, "both rounds ran");
        assert_eq!(progress.rounds.len(), 2);

        assert_eq!(progress.rounds[0].round, 1);
        assert_eq!(progress.rounds[0].findings_before, 3);
        assert_eq!(progress.rounds[0].findings_after, 2);
        assert!(progress.rounds[0].file_changed);
        assert_eq!(progress.rounds[0].stopped, None, "round 1 does not end it");

        assert_eq!(progress.rounds[1].round, 2);
        assert_eq!(progress.rounds[1].findings_before, 2);
        assert_eq!(progress.rounds[1].findings_after, 1);
        assert_eq!(progress.rounds[1].stopped, Some(RoundStop::Budget));

        // And the counters say how the findings fell: by marking, not by
        // modelling. Without this the two are indistinguishable.
        assert_eq!(progress.rounds[0].markings_before, Some(0));
        assert_eq!(progress.rounds[0].markings_after, Some(1));
        assert_eq!(progress.rounds[1].markings_before, Some(1));
        assert_eq!(progress.rounds[1].markings_after, Some(2));
    }

    #[tokio::test]
    async fn a_round_that_changes_nothing_ends_the_chain() {
        /// Answers nothing and writes nothing: the file is byte-identical
        /// afterwards.
        struct SilentRunner {
            calls: std::sync::Mutex<usize>,
        }

        #[async_trait::async_trait]
        impl LlmRunner for SilentRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                _yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                #[allow(clippy::unwrap_used)]
                {
                    *self.calls.lock().unwrap() += 1;
                }
                Ok(())
            }
        }

        let (dir, path) = silent_law_on_disk(3).await;
        let config = rounds_config(FeedbackRounds::uniform(3));
        let runner = SilentRunner {
            calls: std::sync::Mutex::new(0),
        };
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");

        let progress = run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();

        assert_eq!(
            *runner.calls.lock().unwrap(),
            1,
            "three rounds were allowed; the first bought nothing, so no second"
        );
        assert_eq!(progress.rounds.len(), 1);
        assert!(!progress.rounds[0].file_changed);
        assert_eq!(progress.rounds[0].stopped, Some(RoundStop::Unchanged));
        assert_eq!(progress.findings_final, 3);
    }

    #[tokio::test]
    async fn a_round_that_removes_no_finding_ends_the_chain() {
        /// Edits the file without answering anything: the bytes move, the
        /// findings do not.
        struct ChurningRunner {
            calls: std::sync::Mutex<usize>,
        }

        #[async_trait::async_trait]
        impl LlmRunner for ChurningRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                #[allow(clippy::unwrap_used)]
                let call = {
                    let mut calls = self.calls.lock().unwrap();
                    *calls += 1;
                    *calls
                };
                let content = tokio::fs::read_to_string(yaml_abs).await?;
                tokio::fs::write(yaml_abs, format!("{content}# round {call}\n")).await?;
                Ok(())
            }
        }

        let (dir, path) = silent_law_on_disk(3).await;
        let config = rounds_config(FeedbackRounds::uniform(3));
        let runner = ChurningRunner {
            calls: std::sync::Mutex::new(0),
        };
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");

        let progress = run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();

        assert_eq!(
            *runner.calls.lock().unwrap(),
            1,
            "a round that took no finding away buys no next round"
        );
        assert_eq!(progress.rounds.len(), 1);
        assert!(progress.rounds[0].file_changed, "the bytes did move");
        assert_eq!(progress.rounds[0].findings_after, 3);
        assert_eq!(progress.rounds[0].stopped, Some(RoundStop::NoDecrease));
    }

    #[tokio::test]
    async fn a_gate_with_nothing_to_say_runs_no_round() {
        // The budget is not a quota: a clear gate spends none of it.
        let (dir, path) = silent_law_on_disk(1).await;
        let config = rounds_config(FeedbackRounds::uniform(3));
        let runner = MarkingRunner {
            calls: std::sync::Mutex::new(0),
        };
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");

        // Schema, not marking: this law validates, so that gate is clear.
        let progress = run_feedback_rounds(
            Gate::Schema,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();

        assert_eq!(progress.findings_initial, 0);
        assert!(progress.rounds.is_empty());
        assert_eq!(*runner.calls.lock().unwrap(), 0);
    }

    /// The window a run was given reaches the gate that judges it.
    ///
    /// This is about the wiring, not about `in_window`, which had a test of
    /// its own and was never the broken part. The window used to be read off
    /// `payload.chunk_articles`, and a queue payload never carries one: the
    /// worker writes `None` and the windowed copy exists only inside the
    /// branch that runs the agent. So on every real job the gate was handed
    /// the whole law while the run was about three articles, and reported
    /// findings the agent was under orders not to touch.
    ///
    /// Measured on the payload as the queue delivers it, which is the shape
    /// that was wrong. A test that builds its own windowed payload would have
    /// passed throughout.
    #[tokio::test]
    async fn the_window_a_run_was_given_reaches_the_gate() {
        let (dir, path) = silent_law_on_disk(3).await;
        let config = rounds_config(FeedbackRounds::uniform(0));
        let runner = MarkingRunner {
            calls: std::sync::Mutex::new(0),
        };
        // Exactly what the worker enqueues: chunk_articles is None.
        let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");
        assert!(
            payload.chunk_articles.is_none(),
            "the queue payload carries no window; that is the premise"
        );

        let whole_law = run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .unwrap();
        assert_eq!(
            whole_law.findings_initial, 3,
            "all three articles are silent"
        );

        // The same job, now told which article it is about.
        let window = vec!["2".to_string()];
        let windowed = run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            Some(&window),
            &config,
            &runner,
        )
        .await
        .unwrap();
        assert_eq!(
            windowed.findings_initial, 1,
            "only the article this run is about, got {:?}",
            windowed.findings_initial
        );
    }

    // ---- one session per window -----------------------------------------

    fn feedback_pass(gate: Gate) -> Pass {
        Pass::Feedback(Feedback {
            gate,
            findings: vec!["[schema] art. 1: iets".to_string()],
        })
    }

    #[test]
    fn a_window_opens_its_session_once_and_continues_it() {
        // The first call of the window opens the session under an id the
        // worker chose; every call after it continues that same id. This is
        // the whole point: one window, one reading of the law.
        let session = AgentSession::new(SessionReuse::Window);
        let first = session.plan(&Pass::Translate);
        assert_eq!(first, SessionAction::Start(session.id()));

        session.record("translate", first, None, None);
        assert_eq!(
            session.plan(&feedback_pass(Gate::Schema)),
            SessionAction::Resume(session.id())
        );
        session.record("schema", SessionAction::Resume(session.id()), None, None);
        assert_eq!(
            session.plan(&feedback_pass(Gate::Marking)),
            SessionAction::Resume(session.id()),
            "in window mode the soft gates continue the session too"
        );
    }

    #[test]
    fn a_new_window_is_a_new_session() {
        // Nothing carries a session across windows: the agent that enriched
        // the first chunk of the Awir must not walk into the last one still
        // holding everything it wrote.
        let first = AgentSession::new(SessionReuse::Window);
        first.record("translate", first.plan(&Pass::Translate), None, None);
        assert!(first.plan(&feedback_pass(Gate::Checks)).resumed());

        let second = AgentSession::new(SessionReuse::Window);
        assert_ne!(second.id(), first.id(), "each window has its own id");
        assert_eq!(
            second.plan(&Pass::Translate),
            SessionAction::Start(second.id()),
            "a fresh window starts cold, whatever the previous one did"
        );
    }

    #[test]
    fn repair_mode_keeps_the_judgement_gates_cold() {
        // The schema gate repairs a fact and may remember; the checks and
        // marking gates ask the agent to look again at a choice it made, and
        // an agent that remembers making it is the failure those gates exist
        // to catch.
        let session = AgentSession::new(SessionReuse::Repair);
        session.record("translate", session.plan(&Pass::Translate), None, None);

        assert_eq!(
            session.plan(&feedback_pass(Gate::Schema)),
            SessionAction::Resume(session.id())
        );
        assert_eq!(
            session.plan(&feedback_pass(Gate::Checks)),
            SessionAction::Cold
        );
        assert_eq!(
            session.plan(&feedback_pass(Gate::Marking)),
            SessionAction::Cold
        );
    }

    #[test]
    fn reuse_off_never_shares_a_session() {
        let session = AgentSession::new(SessionReuse::Off);
        assert_eq!(session.plan(&Pass::Translate), SessionAction::Cold);
        session.record("translate", SessionAction::Cold, None, None);
        assert_eq!(
            session.plan(&feedback_pass(Gate::Schema)),
            SessionAction::Cold
        );
    }

    #[test]
    fn a_failed_first_call_leaves_the_session_unopened() {
        // `record` only runs after the subprocess succeeded. Without a
        // successful start there is no conversation on disk, so the next call
        // must start rather than resume an id the provider never wrote.
        let session = AgentSession::new(SessionReuse::Window);
        let planned = session.plan(&Pass::Translate);
        assert_eq!(planned, SessionAction::Start(session.id()));
        // …call fails, nothing recorded…
        assert_eq!(
            session.plan(&feedback_pass(Gate::Schema)),
            SessionAction::Start(session.id())
        );
    }

    #[test]
    fn session_reuse_parses_its_three_modes() {
        assert_eq!(SessionReuse::parse("off").unwrap(), SessionReuse::Off);
        assert_eq!(SessionReuse::parse("repair").unwrap(), SessionReuse::Repair);
        assert_eq!(
            SessionReuse::parse(" window ").unwrap(),
            SessionReuse::Window
        );
        assert_eq!(SessionReuse::default(), SessionReuse::Window);
        assert!(SessionReuse::parse("sometimes").is_err());
    }

    #[test]
    fn a_tool_the_lane_does_not_grant_is_denied_and_not_merely_left_off() {
        // `--allowedTools` auto-approves; it does not withhold. Leaving Bash
        // off it is what let the enrichment agent make twenty shell calls in a
        // session whose plan reported the shell as absent. Whatever the
        // capability planner calls ungranted has to arrive as `--disallowedTools`
        // or the plan is a description of a runtime that does not exist.
        let provider = LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        };
        let args = |allow_bash: bool, deny: &[String]| -> Vec<String> {
            build_command(
                &provider,
                "prompt",
                None,
                Path::new("/tmp"),
                ToolPolicy { allow_bash, deny },
                None,
                SessionAction::Cold,
            )
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
        };

        let lane = args(false, &["WebFetch".to_owned(), "WebSearch".to_owned()]);
        let at = lane
            .iter()
            .position(|a| a == "--disallowedTools")
            .expect("lane denies something");
        assert_eq!(lane[at + 1], "Bash,WebFetch,WebSearch");

        // A caller that asked for the shell keeps it.
        let converter = args(true, &[]);
        assert!(
            !converter.iter().any(|a| a == "--disallowedTools"),
            "document-convert must keep its shell: {converter:?}"
        );
    }

    #[test]
    fn the_claude_command_starts_and_resumes_a_session() {
        let provider = LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        };
        let id = Uuid::new_v4();
        let args = |action: SessionAction| -> Vec<String> {
            build_command(
                &provider,
                "prompt",
                None,
                Path::new("/tmp"),
                ToolPolicy {
                    allow_bash: false,
                    deny: &[],
                },
                None,
                action,
            )
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
        };

        let cold = args(SessionAction::Cold);
        assert!(!cold.iter().any(|a| a == "--session-id" || a == "--resume"));

        let start = args(SessionAction::Start(id));
        let at = start
            .iter()
            .position(|a| a == "--session-id")
            .expect("start");
        assert_eq!(start[at + 1], id.to_string());

        let resume = args(SessionAction::Resume(id));
        let at = resume.iter().position(|a| a == "--resume").expect("resume");
        assert_eq!(resume[at + 1], id.to_string());
    }

    /// Answers a round like [`MarkingRunner`], but takes the session decision
    /// the same way the process runner does — through `begin_call`/`end_call`
    /// — and writes down what it decided. Anything less would test a copy of
    /// the rule rather than the rule.
    struct SessionRunner {
        inner: MarkingRunner,
        actions: std::sync::Mutex<Vec<(String, SessionAction)>>,
    }

    impl SessionRunner {
        fn new() -> Self {
            Self {
                inner: MarkingRunner {
                    calls: std::sync::Mutex::new(0),
                },
                actions: std::sync::Mutex::new(Vec::new()),
            }
        }

        #[allow(clippy::unwrap_used)]
        fn actions(&self) -> Vec<(String, SessionAction)> {
            self.actions.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LlmRunner for SessionRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            repo_path: &Path,
            config: &EnrichConfig,
        ) -> Result<()> {
            let action = begin_call(payload);
            #[allow(clippy::unwrap_used)]
            {
                self.actions
                    .lock()
                    .unwrap()
                    .push((pass_label(&payload.pass).to_string(), action));
            }
            self.inner.run(payload, yaml_abs, repo_path, config).await?;
            end_call(payload, action, AgentRun::default());
            Ok(())
        }
    }

    async fn rounds_with_session(
        reuse: SessionReuse,
        session: &std::sync::Arc<AgentSession>,
    ) -> Vec<(String, SessionAction)> {
        let (dir, path) = silent_law_on_disk(3).await;
        let mut config = rounds_config(FeedbackRounds {
            marking: 2,
            ..FeedbackRounds::default()
        });
        config.session_reuse = reuse;
        let payload = EnrichPayload {
            session: Some(std::sync::Arc::clone(session)),
            ..chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml")
        };
        let runner = SessionRunner::new();

        run_feedback_rounds(
            Gate::Marking,
            &path,
            dir.path(),
            &payload,
            None,
            &config,
            &runner,
        )
        .await
        .expect("rounds run");

        runner.actions()
    }

    #[tokio::test]
    async fn a_second_round_in_the_same_window_continues_the_session() {
        // Two rounds at the same gate, over the same articles: the second
        // must continue the first rather than start a seventh cold process.
        let session = std::sync::Arc::new(AgentSession::new(SessionReuse::Window));
        let actions = rounds_with_session(SessionReuse::Window, &session).await;

        assert_eq!(actions.len(), 2, "both rounds ran");
        assert_eq!(
            actions[0],
            ("marking".to_string(), SessionAction::Start(session.id()))
        );
        assert_eq!(
            actions[1],
            ("marking".to_string(), SessionAction::Resume(session.id()))
        );

        // And both calls are accounted, so a run can be read per call.
        let calls = session.calls();
        assert_eq!(calls.len(), 2);
        assert!(!calls[0].resumed);
        assert!(calls[1].resumed);
        assert_eq!(calls[1].step, "marking");
    }

    #[tokio::test]
    async fn the_next_window_does_not_continue_the_previous_one() {
        // Same law, same gate, a new window: nothing is resumed. The saving
        // stops at the window boundary on purpose.
        let first = std::sync::Arc::new(AgentSession::new(SessionReuse::Window));
        let first_actions = rounds_with_session(SessionReuse::Window, &first).await;
        assert!(first_actions.iter().any(|(_, a)| a.resumed()));

        let second = std::sync::Arc::new(AgentSession::new(SessionReuse::Window));
        let second_actions = rounds_with_session(SessionReuse::Window, &second).await;
        assert_eq!(
            second_actions[0].1,
            SessionAction::Start(second.id()),
            "a new window opens its own session"
        );
        assert_ne!(second.id(), first.id());
    }

    #[tokio::test]
    async fn the_marking_gate_stays_cold_under_repair_mode() {
        let session = std::sync::Arc::new(AgentSession::new(SessionReuse::Repair));
        let actions = rounds_with_session(SessionReuse::Repair, &session).await;

        assert_eq!(actions.len(), 2);
        assert!(
            actions.iter().all(|(_, a)| *a == SessionAction::Cold),
            "repair mode gives the marking gate a fresh process, got {actions:?}"
        );
    }

    #[test]
    fn the_window_total_is_the_sum_of_its_calls() {
        // Cost per window and per call come from the same record, so the
        // total can never disagree with the lines above it.
        let session = AgentSession::new(SessionReuse::Window);
        assert_eq!(session.total(), None, "no call, no figure");

        let translate = AgentUsage {
            input_tokens: 100,
            output_tokens: 20,
            cache_read_tokens: 900,
            cache_write_tokens: 0,
            cost_millicents: 1500,
        };
        let repair = AgentUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 400,
            cache_write_tokens: 0,
            cost_millicents: 300,
        };
        session.record(
            "translate",
            SessionAction::Start(session.id()),
            Some(translate),
            None,
        );
        session.record(
            "schema",
            SessionAction::Resume(session.id()),
            Some(repair),
            None,
        );

        let total = session.total().expect("two calls reported");
        assert_eq!(total.input_tokens, 110);
        assert_eq!(total.output_tokens, 25);
        assert_eq!(total.cache_read_tokens, 1300);
        assert_eq!(total.cost_millicents, 1800);
    }

    #[test]
    fn feedback_rounds_count_every_call_they_allow() {
        // One round per gate is the ceiling the constant names.
        assert_eq!(
            FeedbackRounds::default().max_calls(),
            MAX_AGENT_CALLS_PER_RUN - 1
        );
        assert_eq!(FeedbackRounds::uniform(2).max_calls(), 12);
        // The schema budget counts twice: per window and after the closing pass.
        let schema_only = FeedbackRounds {
            schema: 3,
            ..FeedbackRounds::uniform(0)
        };
        assert_eq!(schema_only.max_calls(), 6);
        let rest = FeedbackRounds {
            schema: 0,
            ..FeedbackRounds::uniform(1)
        };
        assert_eq!(rest.max_calls(), 4);
    }

    #[test]
    fn feedback_rounds_default_to_one_per_gate() {
        let rounds = FeedbackRounds::default();
        for gate in Gate::ALL {
            assert_eq!(rounds.for_gate(gate), 1, "{}", gate.label());
        }
    }

    #[test]
    fn feedback_rounds_parse_bare_number_and_per_gate() {
        assert_eq!(
            FeedbackRounds::parse("2").unwrap(),
            FeedbackRounds::uniform(2)
        );
        assert_eq!(
            FeedbackRounds::parse("checks=2,marking=3,binding=4").unwrap(),
            FeedbackRounds {
                schema: 1,
                checks: 2,
                marking: 3,
                reconcile: 1,
                binding: 4,
            }
        );
        // A bare number sets the floor, a named gate overrides it — order
        // matters and the number goes first.
        assert_eq!(
            FeedbackRounds::parse("2, schema=1").unwrap(),
            FeedbackRounds {
                schema: 1,
                checks: 2,
                marking: 2,
                reconcile: 2,
                binding: 2,
            }
        );
        assert!(FeedbackRounds::parse("poort=2").is_err());
        assert!(FeedbackRounds::parse("checks=veel").is_err());
    }

    #[tokio::test]
    async fn test_count_article_stats_empty_vs_null_machine_readable() {
        // An empty `machine_readable: {}` mapping counts as enriched — this
        // matches the old key-presence semantics that `FakeLlmRunner` (and the
        // enrichment delta) rely on when the LLM inserts a bare section. An
        // explicit `machine_readable: null` deserializes to None and is treated
        // as un-enriched; no corpus file uses the bare/null form, so the typed
        // count matches the previous `contains_key` behavior in practice.
        let yaml = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
articles:
  - number: '1'
    text: Empty section, enriched.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
    machine_readable: {}
  - number: '2'
    text: Null section, not enriched.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
    machine_readable: null
  - number: '3'
    text: No section at all.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel3
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("law.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();

        let (total, with_mr) = count_article_stats(&path).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(with_mr, 1);
    }

    const MINIMAL_LAW: &str = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
    machine_readable:
      markings:
        - about: fixture
          reason: het formaat kent hier geen vorm voor deze constructie
          resolution: model
          target: []
          legal_text_excerpt: Article one.
"#;

    /// Writes something the schema rejects on the first run and repairs it
    /// when handed the errors, which is the shape of the repair round.
    struct InvalidThenRepairingRunner {
        calls: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    }

    #[async_trait::async_trait]
    impl LlmRunner for InvalidThenRepairingRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let is_repair = matches!(payload.pass, Pass::Feedback(_));
            #[allow(clippy::unwrap_used)]
            self.calls.lock().unwrap().push(is_repair);

            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut m) = article {
                            let mut mr = serde_yaml_ng::Mapping::new();
                            if is_repair {
                                // The repair round drops the invented key.
                                mr.insert(
                                    "markings".into(),
                                    serde_yaml_ng::Value::Sequence(vec![]),
                                );
                            } else {
                                // An invented key: `machine_readable` is
                                // `additionalProperties: false`.
                                mr.insert("verzonnen_sleutel".into(), "iets".into());
                            }
                            m.insert("machine_readable".into(), serde_yaml_ng::Value::Mapping(mr));
                        }
                    }
                }
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&value)?).await?;
            Ok(())
        }
    }

    /// The gate that the `law-generate` skill asks for and the runtime cannot
    /// give it: schema validation, with one round to put it right.
    #[tokio::test]
    async fn invalid_output_gets_one_repair_round() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        tokio::fs::write(&path, MINIMAL_LAW).await.unwrap();

        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let runner = InvalidThenRepairingRunner {
            calls: calls.clone(),
        };
        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: "nl/wet/test_law/2025-01-01.yaml".into(),
            ..Default::default()
        };
        let config = EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });

        let result = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner).await;
        assert!(
            result.is_ok(),
            "repair round should have fixed it: {result:?}"
        );

        let calls = calls.lock().unwrap().clone();
        // The translation, then the schema repair. A third call follows from
        // the soft marking gate, which asks about the article that the fake
        // runner leaves without an outcome; this test is about the repair.
        assert_eq!(calls.first(), Some(&false), "the first call translates");
        assert_eq!(
            calls.get(1),
            Some(&true),
            "expected one enrichment run and one repair run"
        );

        // And the file on disk is valid, which is the whole point.
        let yaml = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(crate::enrich_v2::checks::schema_errors(&yaml).is_empty());
    }

    /// A soft gate hands its findings over and accepts a marking as an
    /// answer. What survives the round is recorded rather than fatal:
    /// failing here would turn every open norm into a defect.
    #[tokio::test]
    async fn a_soft_gate_does_not_fail_the_job() {
        /// Writes a model that leaves a coverage question standing: the text
        /// derogates and the model has no branch, which is exactly what the
        /// checks report and what the agent may answer with a marking.
        struct UnansweringRunner {
            passes: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
        }

        #[async_trait::async_trait]
        impl LlmRunner for UnansweringRunner {
            async fn run(
                &self,
                payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                #[allow(clippy::unwrap_used)]
                self.passes.lock().unwrap().push(match &payload.pass {
                    Pass::Translate => "translate",
                    Pass::Feedback(f) => match f.gate {
                        Gate::Schema => "schema",
                        Gate::Checks => "checks",
                        Gate::Marking => "marking",
                        Gate::Reconcile => "reconcile",
                        Gate::Binding => "binding",
                    },
                });
                if !matches!(payload.pass, Pass::Translate) {
                    return Ok(()); // answers nothing, on purpose
                }
                let content = tokio::fs::read_to_string(yaml_abs).await?;
                let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
                if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                    if let Some(serde_yaml_ng::Value::Sequence(ref mut arts)) =
                        map.get_mut("articles")
                    {
                        for a in arts.iter_mut() {
                            if let serde_yaml_ng::Value::Mapping(ref mut m) = a {
                                m.insert(
                                    "machine_readable".into(),
                                    serde_yaml_ng::Value::Mapping(Default::default()),
                                );
                            }
                        }
                    }
                }
                tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&value)?).await?;
                Ok(())
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        // A lid that derogates: the checks will say the model has no branch.
        let law = MINIMAL_LAW.replace(
            "    text: Article one.",
            "    text: |-\n      1. De hoofdregel geldt.\n\n      2. In afwijking van het eerste lid geldt de helft.",
        );
        tokio::fs::write(&path, law).await.unwrap();

        let passes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let runner = UnansweringRunner {
            passes: passes.clone(),
        };
        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: "nl/wet/test_law/2025-01-01.yaml".into(),
            ..Default::default()
        };
        let config = EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });

        let result = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner).await;
        assert!(
            result.is_ok(),
            "a surviving coverage finding must not fail the job: {result:?}"
        );

        let passes = passes.lock().unwrap().clone();
        assert!(
            passes.contains(&"checks"),
            "the checks gate must have handed its findings over: {passes:?}"
        );
        assert!(
            !passes.contains(&"schema"),
            "the schema gate had nothing to say: {passes:?}"
        );
    }

    /// A run that changed nothing must not be validated: the file was already
    /// in whatever state it was in, and failing here would fail a job for
    /// someone else's defect.
    #[tokio::test]
    async fn an_unchanged_file_is_not_validated() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let path = law_dir.join("2025-01-01.yaml");
        // Deliberately invalid: no `$schema` at all.
        tokio::fs::write(
            &path,
            "---\n$id: test_law\narticles:\n  - number: '1'\n    text: Iets.\n",
        )
        .await
        .unwrap();

        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: "nl/wet/test_law/2025-01-01.yaml".into(),
            ..Default::default()
        };
        let config = EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });

        let result =
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &NoopLlmRunner).await;
        // It fails on producing nothing, not on the pre-existing schema state.
        let err = format!("{result:?}");
        assert!(!err.contains("schema error"), "{err}");
    }

    /// A run that walks no window must not move the cursor.
    ///
    /// `--steps reconcile` runs only the closing pass: no translation, no
    /// agent over a window, nothing an article has been read for. The cursor
    /// the chunk planner computes is the cursor of the window that *would*
    /// have been walked, and writing it pushes the ordinary walk past articles
    /// no agent has seen — the same defect a targeted run is explicitly
    /// protected against (RFC-026, "De cursor blijft staan").
    ///
    /// Asserted on the sidecar rather than on the returned result, because the
    /// sidecar is what the next run reads.
    #[tokio::test]
    async fn a_reconcile_only_run_leaves_the_cursor_where_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let rel = "nl/wet/test_law/2025-01-01.yaml";
        let mut body = String::from(
            "---\n\
             $schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json\n\
             $id: test_law\n\
             regulatory_layer: WET\n\
             publication_date: '2025-01-01'\n\
             bwb_id: BWBR0000001\n\
             url: https://wetten.overheid.nl/BWBR0000001/2025-01-01\n\
             valid_from: '2025-01-01'\n\
             articles:\n",
        );
        for n in 1..=6 {
            body.push_str(&format!("  - number: '{n}'\n    text: Artikel {n}.\n"));
        }
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), &body)
            .await
            .unwrap();

        // What the ordinary walk had reached: two of six articles. Written as
        // the three fields the cursor reader looks for, which is all a sidecar
        // has to carry to stand in for a previous run.
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            format!(
                "enrich_cursor: 2\nenrich_cursor_path: {rel}\nenrich_cursor_mode: {}\n",
                WindowMode::default().label()
            ),
        )
        .await
        .unwrap();

        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: rel.into(),
            ..Default::default()
        };
        let mut config = EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        // Chunking on, so the planner has a window to compute a cursor from,
        // and the window step off, so no window is walked.
        config.max_articles_per_run = 3;
        config.steps = RunSteps {
            window: false,
            reconcile: true,
        };

        let outcome =
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &NoopLlmRunner).await;
        eprintln!("OUTCOME: {outcome:?}");
        eprintln!(
            "SIDECAR: {:?}",
            std::fs::read_to_string(law_dir.join(".enrichment.yaml"))
        );

        let (cursor, path, _) = read_stored_cursor(dir.path(), rel).await;
        assert_eq!(
            (cursor, path.as_str()),
            (2, rel),
            "de reconcile-run vertaalde geen venster, dus de wandeling schuift niet op"
        );
    }

    /// Fake LLM runner that simulates enrichment by adding `machine_readable`
    /// sections to articles that don't already have them.
    struct FakeLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for FakeLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;

            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut article_map) = article {
                            if !article_map.contains_key("machine_readable") {
                                article_map.insert(
                                    serde_yaml_ng::Value::String("machine_readable".into()),
                                    serde_yaml_ng::Value::Mapping(Default::default()),
                                );
                            }
                        }
                    }
                }
            }

            let output = serde_yaml_ng::to_string(&value)?;
            tokio::fs::write(yaml_abs, output).await?;
            Ok(())
        }
    }

    /// Three articles, one of them already translated.
    const FAKE_RUNNER_LAW: &str = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
  - number: '2'
    text: Article two.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
    machine_readable:
      execution:
        actions: []
  - number: '3'
    text: Article three.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel3
"#;

    /// Enriches like [`FakeLlmRunner`], and on the way cuts the text of
    /// article 1 short.
    struct TruncatingLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for TruncatingLlmRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            repo_path: &Path,
            config: &EnrichConfig,
        ) -> Result<()> {
            FakeLlmRunner
                .run(payload, yaml_abs, repo_path, config)
                .await?;
            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let Some(serde_yaml_ng::Value::Mapping(first)) =
                doc.get_mut("articles").and_then(|a| a.get_mut(0))
            {
                first.insert("text".into(), "Article".into());
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&doc)?).await?;
            Ok(())
        }
    }

    /// The article count catches a deleted entry and says nothing about the
    /// text inside one. An agent that shortened an article moved no counter
    /// at all, so the file was committed with a wrong legal text under a run
    /// reporting success.
    #[tokio::test]
    async fn a_shortened_article_text_fails_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        let law_abs = dir.path().join(yaml_path);
        tokio::fs::create_dir_all(law_abs.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&law_abs, FAKE_RUNNER_LAW).await.unwrap();

        let payload = chunk_test_payload(yaml_path);
        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err =
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &TruncatingLlmRunner)
                .await
                .expect_err("a truncated article text must fail the run");
        let message = err.to_string();
        assert!(
            message.contains("the statutory text of article 1 changed"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_with_fake_runner() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = FAKE_RUNNER_LAW;
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: Some("opencode".into()),
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let (result, written_files) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &FakeLlmRunner)
                .await
                .unwrap();

        assert_eq!(result.articles_total, 3);
        assert_eq!(result.articles_with_machine_readable, 3);
        // 2 out of 2 articles needing enrichment were enriched
        assert!((result.coverage_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(result.provider, "opencode");
        assert_eq!(result.branch, "enrich/opencode");

        // Should have written the YAML file + metadata file
        assert!(written_files.len() >= 2);

        // Verify metadata file was written
        let metadata_path = law_dir.join(".enrichment.yaml");
        assert!(metadata_path.exists());
        let meta_content = tokio::fs::read_to_string(&metadata_path).await.unwrap();
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(&meta_content).unwrap();
        assert_eq!(meta.law_id, "BWBR0000001");
        assert_eq!(meta.provider, "opencode");
        assert_eq!(meta.articles_with_machine_readable, 3);
    }

    #[tokio::test]
    async fn one_run_shares_one_session_and_accounts_every_call() {
        // The window boundary is the run: `execute_enrich_with_runner` opens
        // one session, the translation pass and the gates after it share it,
        // and a second run over the same law opens another. Everything the
        // measurement needs rides back in the result.
        let mut config = rounds_config(FeedbackRounds::default());
        config.session_reuse = SessionReuse::Window;

        let ids_of = |actions: Vec<(String, SessionAction)>| -> Vec<Uuid> {
            actions
                .into_iter()
                .filter_map(|(_, a)| match a {
                    SessionAction::Start(id) | SessionAction::Resume(id) => Some(id),
                    SessionAction::Cold => None,
                })
                .collect()
        };

        let run = |config: EnrichConfig| async move {
            let (dir, _) = silent_law_on_disk(3).await;
            let payload = chunk_test_payload("regulation/nl/wet/test_law/2025-01-01.yaml");
            let runner = SessionRunner::new();
            let (result, _) =
                execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
                    .await
                    .expect("run");
            (result, runner.actions())
        };

        let (first_result, first_actions) = run(config.clone()).await;
        let first_ids = ids_of(first_actions);
        assert!(
            first_ids.len() > 1,
            "the run made several calls; got {first_ids:?}"
        );
        assert!(
            first_ids.windows(2).all(|w| w[0] == w[1]),
            "every call in one window carries the same session: {first_ids:?}"
        );
        assert_eq!(first_result.session_reuse, "window");
        assert_eq!(
            first_result.agent_calls.len(),
            first_ids.len(),
            "every call is accounted, resumed or not"
        );
        assert!(first_result.agent_calls[0].step == "translate");
        assert!(
            first_result.agent_calls.iter().skip(1).any(|c| c.resumed),
            "the gates after the translation continue the session"
        );

        let (_, second_actions) = run(config).await;
        let second_ids = ids_of(second_actions);
        assert_ne!(
            second_ids.first(),
            first_ids.first(),
            "a second run is a second window and opens its own session"
        );
    }

    /// Fake runner that fails, to test error path.
    struct FailingLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for FailingLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            _yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            Err(PipelineError::Enrich("simulated LLM failure".into()))
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_with_failing_runner() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = "---\n$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json\n$id: test_law\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Article one.\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, "", &FailingLlmRunner)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("simulated LLM failure"));
    }

    /// Runner that succeeds but doesn't modify the file — should fail with
    /// zero-coverage error.
    struct NoopLlmRunner;

    #[async_trait::async_trait]
    impl LlmRunner for NoopLlmRunner {
        async fn run(
            &self,
            _payload: &EnrichPayload,
            _yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_zero_coverage_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();

        let yaml_content = "---\n$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json\n$id: test_law\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Article one.\n";
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), yaml_content)
            .await
            .unwrap();

        let payload = EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, "", &NoopLlmRunner)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no machine_readable sections"));
    }

    // --- Chunked enrichment ---

    #[test]
    fn plan_chunk_zero_disables_chunking() {
        assert_eq!(
            plan_chunk(0, 324, 0, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::WholeLaw
        );
        // Even a stored cursor is ignored in whole-law mode.
        assert_eq!(
            plan_chunk(0, 324, 100, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::WholeLaw
        );
    }

    #[test]
    fn plan_chunk_first_run_starts_at_zero() {
        // Legacy metadata (no cursor fields) reads as (0, "") — path mismatch
        // resets to 0, which is also the correct start.
        assert_eq!(
            plan_chunk(15, 324, 0, "", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resumes_from_valid_cursor() {
        assert_eq!(
            plan_chunk(15, 324, 30, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 30,
                end: 45,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resets_on_path_mismatch() {
        // A new law version lives at a different (date-named) path: the cursor
        // recorded for the old version must not apply.
        assert_eq!(
            plan_chunk(
                15,
                324,
                30,
                "regulation/2025-01-01.yaml",
                "regulation/2026-01-01.yaml",
                &[]
            ),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_resets_on_cursor_beyond_total() {
        // Corrupt metadata or a shrunk document: a cursor past the end resets.
        assert_eq!(
            plan_chunk(15, 20, 25, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 0,
                end: 15,
                law_complete: false
            }
        );
    }

    #[test]
    fn plan_chunk_final_window_is_complete() {
        // Partial last window.
        assert_eq!(
            plan_chunk(15, 20, 15, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 15,
                end: 20,
                law_complete: true
            }
        );
        // Window exactly reaching the end.
        assert_eq!(
            plan_chunk(10, 20, 10, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 10,
                end: 20,
                law_complete: true
            }
        );
        // Law smaller than one window: complete in a single run.
        assert_eq!(
            plan_chunk(15, 3, 0, "", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 0,
                end: 3,
                law_complete: true
            }
        );
    }

    #[test]
    fn plan_chunk_cursor_at_end_yields_empty_complete_window() {
        // cursor == total is valid (the loop finished earlier): empty window,
        // trivially complete — execute skips the LLM run entirely.
        assert_eq!(
            plan_chunk(15, 20, 20, "regulation/a.yaml", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 20,
                end: 20,
                law_complete: true
            }
        );
    }

    #[test]
    fn plan_chunk_empty_law_is_complete() {
        assert_eq!(
            plan_chunk(15, 0, 0, "", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 0,
                end: 0,
                law_complete: true
            }
        );
    }

    fn numbers(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    /// The chapeau of an article and its leden belong in one window. Without
    /// this rule a window of two cuts article 1 after its first onderdeel, and
    /// the entry that binds the leden together is written before they exist.
    #[test]
    fn plan_chunk_never_cuts_a_top_level_article_in_half() {
        let entries = numbers(&["1", "1.1", "1.2", "1.3", "2", "2.1"]);
        assert_eq!(
            plan_chunk(2, entries.len(), 0, "", "regulation/a.yaml", &entries),
            ChunkPlan::Chunk {
                start: 0,
                end: 4,
                law_complete: false
            },
            "het venster schuift door tot artikel 1 op is"
        );
        // And the next window starts on the boundary the previous one left.
        assert_eq!(
            plan_chunk(
                2,
                entries.len(),
                4,
                "regulation/a.yaml",
                "regulation/a.yaml",
                &entries
            ),
            ChunkPlan::Chunk {
                start: 4,
                end: 6,
                law_complete: true
            }
        );
    }

    /// The snap only ever grows a window, so a run still consumes at least
    /// `max_articles_per_run` entries and the walk still terminates in
    /// `ceil(total / N)` runs.
    #[test]
    fn plan_chunk_snapping_never_shrinks_a_window() {
        let entries = numbers(&["1", "1.1", "1.2", "2", "2.1", "2.2", "2.3", "3", "4", "4.1"]);
        let mut cursor = 0usize;
        let mut runs = 0usize;
        while cursor < entries.len() {
            let ChunkPlan::Chunk { start, end, .. } = plan_chunk(
                3,
                entries.len(),
                cursor,
                "regulation/a.yaml",
                "regulation/a.yaml",
                &entries,
            ) else {
                unreachable!("chunking is on")
            };
            assert_eq!(start, cursor);
            assert!(end - start >= 3, "een venster wordt nooit kleiner dan N");
            cursor = end;
            runs += 1;
        }
        assert!(runs <= entries.len().div_ceil(3));
    }

    /// A caller that hands no numbers gets exactly the behaviour from before
    /// the rule existed.
    #[test]
    fn plan_chunk_without_numbers_is_the_old_behaviour() {
        let entries = numbers(&["1", "1.1", "1.2", "1.3"]);
        assert_eq!(
            plan_chunk(2, entries.len(), 0, "", "regulation/a.yaml", &[]),
            ChunkPlan::Chunk {
                start: 0,
                end: 2,
                law_complete: false
            }
        );
    }

    // ---- the layer as a window, and the knob inside it -------------------

    fn graph_of(yaml: &str) -> crate::enrich_v2::refgraph::Graph {
        crate::enrich_v2::refgraph::Graph::scan(&serde_yaml_ng::from_str(yaml).unwrap())
    }

    /// A law whose article 3 is read by article 1, and whose article 2 is
    /// unrelated to both. Three layers is the wrong answer here; two is right,
    /// with 2 and 3 together in the first.
    const LAYERED_LAW: &str = r"bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De hoogte volgt uit artikel 3.
  - number: '1.1'
    text: Het eerste lid geldt onverkort.
  - number: '2'
    text: Deze wet treedt in werking met ingang van 1 januari.
  - number: '3'
    text: Het bedrag is duizend euro.
";

    #[test]
    fn a_layer_is_a_window_whose_size_follows_from_the_law() {
        let graph = graph_of(LAYERED_LAW);
        let entries = numbers(&["1", "1.1", "2", "3"]);
        let (first, complete) = plan_layer_window(&graph, &entries, 0);
        assert!(!complete);
        assert_eq!(first, numbers(&["2", "3"]), "de producenten gaan voor");
        let (second, complete) = plan_layer_window(&graph, &entries, 1);
        assert!(complete, "de laatste laag sluit de wet af");
        assert_eq!(
            second,
            numbers(&["1", "1.1"]),
            "een aanhef reist met zijn eigen leden mee"
        );
    }

    /// Every layer is walked once and the cursor counts them, so the walk ends
    /// in a fixed number of runs — the property the entry cursor had.
    #[test]
    fn the_layer_walk_covers_every_entry_exactly_once_and_terminates() {
        let graph = graph_of(LAYERED_LAW);
        let entries = numbers(&["1", "1.1", "2", "3"]);
        let mut seen: Vec<String> = Vec::new();
        let mut index = 0usize;
        loop {
            let (window, complete) = plan_layer_window(&graph, &entries, index);
            seen.extend(window);
            index += 1;
            assert!(index <= entries.len(), "de wandeling loopt niet door");
            if complete {
                break;
            }
        }
        seen.sort();
        let mut expected = entries.clone();
        expected.sort();
        assert_eq!(seen, expected);
    }

    /// Default 1: the window stays whole, whatever the graph says.
    #[test]
    fn one_agent_per_window_is_the_default() {
        let graph = graph_of(LAYERED_LAW);
        let window = numbers(&["2", "3"]);
        assert_eq!(split_window(&graph, &window, 1), vec![window.clone()]);
    }

    /// Independent articles may be split; a related pair never is, because
    /// two agents would each have to invent a name for the same concept.
    #[test]
    fn a_window_splits_only_where_nothing_references_anything() {
        let graph = graph_of(LAYERED_LAW);
        assert_eq!(
            split_window(&graph, &numbers(&["2", "3"]), 2),
            vec![numbers(&["2"]), numbers(&["3"])]
        );
        // Article 1 reads article 3: one agent, or the name is a guess.
        assert_eq!(
            split_window(&graph, &numbers(&["1", "1.1", "3"]), 2),
            vec![numbers(&["1", "1.1", "3"])]
        );
        // A split never cuts inside a top-level article.
        assert_eq!(
            split_window(&graph, &numbers(&["1", "1.1"]), 2),
            vec![numbers(&["1", "1.1"])]
        );
    }

    #[test]
    fn the_merge_takes_each_window_its_own_entries() {
        let base = LAYERED_LAW;
        let two = base.replace(
            "  - number: '2'\n    text: Deze wet treedt in werking met ingang van 1 januari.\n",
            "  - number: '2'\n    text: Deze wet treedt in werking met ingang van 1 januari.\n    machine_readable: {}\n",
        );
        let three = base.replace(
            "  - number: '3'\n    text: Het bedrag is duizend euro.\n",
            "  - number: '3'\n    text: Het bedrag is duizend euro.\n    machine_readable: {}\n",
        );
        let merged =
            merge_windows(base, &[(numbers(&["2"]), two), (numbers(&["3"]), three)]).unwrap();
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&merged).unwrap();
        assert!(doc["articles"][2].get("machine_readable").is_some());
        assert!(doc["articles"][3].get("machine_readable").is_some());
        assert!(doc["articles"][0].get("machine_readable").is_none());
    }

    /// The guard: a window that wrote outside its assignment fails the run
    /// with the entry number in the message, instead of one agent silently
    /// winning. Round 3 lost four runs to two agents in one file.
    #[test]
    fn the_merge_refuses_when_a_window_wrote_outside_its_assignment() {
        let base = LAYERED_LAW;
        let stray = base.replace(
            "  - number: '1'\n    text: De hoogte volgt uit artikel 3.\n",
            "  - number: '1'\n    text: De hoogte volgt uit artikel 3.\n    machine_readable: {}\n",
        );
        let error = merge_windows(base, &[(numbers(&["2"]), stray)]).unwrap_err();
        assert!(error.contains("entry 1"), "{error}");
    }

    #[test]
    fn the_merge_refuses_when_a_window_changed_the_entry_count() {
        let base = LAYERED_LAW;
        let extra = format!("{base}  - number: '4'\n    text: Een artikel dat er niet was.\n");
        let error = merge_windows(base, &[(numbers(&["2"]), extra)]).unwrap_err();
        assert!(error.contains("number of entries"), "{error}");
    }

    /// Writes an empty `machine_readable` into exactly the entries it was
    /// assigned, and records which checkout it was handed.
    struct WindowRunner {
        checkouts: std::sync::Mutex<Vec<PathBuf>>,
    }

    #[async_trait::async_trait]
    impl LlmRunner for WindowRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            #[allow(clippy::unwrap_used)]
            {
                self.checkouts.lock().unwrap().push(repo_path.to_path_buf());
            }
            let mine = payload.chunk_articles.clone().unwrap_or_default();
            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let Some(serde_yaml_ng::Value::Sequence(articles)) = doc.get_mut("articles") {
                for article in articles.iter_mut() {
                    let is_mine = article
                        .get("number")
                        .and_then(serde_yaml_ng::Value::as_str)
                        .is_some_and(|n| mine.iter().any(|m| m == n));
                    if !is_mine {
                        continue;
                    }
                    if let serde_yaml_ng::Value::Mapping(map) = article {
                        map.insert(
                            "machine_readable".into(),
                            serde_yaml_ng::Value::Mapping(Default::default()),
                        );
                    }
                }
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&doc)?).await?;
            Ok(())
        }
    }

    /// Each agent works in its own checkout and the merge folds the two back
    /// into one file. Nobody writes the file another agent is holding.
    #[tokio::test]
    async fn windows_run_on_their_own_copy_and_are_merged_back() {
        let dir = tempfile::tempdir().unwrap();
        let law_rel = "regulation/nl/wet/test_law/2025-01-01.yaml";
        let law_abs = dir.path().join(law_rel);
        tokio::fs::create_dir_all(law_abs.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&law_abs, LAYERED_LAW).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let payload = corpus_wide_payload();
        let runner = WindowRunner {
            checkouts: std::sync::Mutex::new(Vec::new()),
        };
        let sub_windows = vec![numbers(&["2"]), numbers(&["3"])];
        run_windows_concurrently(
            &sub_windows,
            &payload,
            &law_abs,
            law_rel,
            dir.path(),
            &config,
            &runner,
        )
        .await
        .unwrap();

        let after = tokio::fs::read_to_string(&law_abs).await.unwrap();
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&after).unwrap();
        assert!(doc["articles"][2].get("machine_readable").is_some());
        assert!(doc["articles"][3].get("machine_readable").is_some());
        assert!(doc["articles"][0].get("machine_readable").is_none());

        let checkouts = runner.checkouts.lock().unwrap().clone();
        assert_eq!(checkouts.len(), 2);
        assert_ne!(
            checkouts[0], checkouts[1],
            "elke agent kreeg een eigen kopie"
        );
        assert!(checkouts.iter().all(|c| c != dir.path()));
    }

    /// Does what [`WindowRunner`] does, and writes down the session each
    /// window was handed.
    struct SessionRecordingWindowRunner {
        inner: WindowRunner,
        sessions: std::sync::Mutex<Vec<Option<(uuid::Uuid, SessionReuse)>>>,
    }

    #[async_trait::async_trait]
    impl LlmRunner for SessionRecordingWindowRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            repo_path: &Path,
            config: &EnrichConfig,
        ) -> Result<()> {
            #[allow(clippy::unwrap_used)]
            {
                self.sessions
                    .lock()
                    .unwrap()
                    .push(payload.session.as_ref().map(|s| (s.id(), s.reuse())));
            }
            self.inner.run(payload, yaml_abs, repo_path, config).await
        }
    }

    /// Two windows running side by side must not share a session id. It is
    /// the provider's handle on one conversation, so handing the same one to
    /// two live processes is two agents writing one transcript: the second
    /// refuses, or adopts the first's history and sees articles it was never
    /// assigned. Sharing a session across the calls of one window is the
    /// feature; sharing it across concurrent windows is the opposite of it.
    #[tokio::test]
    async fn concurrent_windows_each_get_their_own_session() {
        let dir = tempfile::tempdir().unwrap();
        let law_rel = "regulation/nl/wet/test_law/2025-01-01.yaml";
        let law_abs = dir.path().join(law_rel);
        tokio::fs::create_dir_all(law_abs.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&law_abs, LAYERED_LAW).await.unwrap();

        let config = test_config(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        let parent = std::sync::Arc::new(AgentSession::new(SessionReuse::Window));
        let payload = EnrichPayload {
            session: Some(std::sync::Arc::clone(&parent)),
            ..corpus_wide_payload()
        };
        let runner = SessionRecordingWindowRunner {
            inner: WindowRunner {
                checkouts: std::sync::Mutex::new(Vec::new()),
            },
            sessions: std::sync::Mutex::new(Vec::new()),
        };
        run_windows_concurrently(
            &[numbers(&["2"]), numbers(&["3"])],
            &payload,
            &law_abs,
            law_rel,
            dir.path(),
            &config,
            &runner,
        )
        .await
        .unwrap();

        let sessions: Vec<_> = runner
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.expect("each window carries a session"))
            .collect();
        assert_eq!(sessions.len(), 2);
        assert_ne!(sessions[0].0, sessions[1].0, "the windows share a session");
        for (id, reuse) in &sessions {
            assert_ne!(*id, parent.id(), "a window was handed the parent's session");
            assert_eq!(*reuse, parent.reuse());
        }
    }

    #[test]
    fn window_mode_parses_and_rejects_what_it_does_not_know() {
        assert_eq!(WindowMode::parse("document").unwrap(), WindowMode::Document);
        assert_eq!(WindowMode::parse("layer").unwrap(), WindowMode::Layer);
        assert_eq!(WindowMode::parse("").unwrap(), WindowMode::Document);
        assert!(WindowMode::parse("laag").is_err());
    }

    /// Plan with every step available, for prompt tests that are not about
    /// capabilities.
    fn full_plan() -> Vec<(&'static capabilities::StepSpec, capabilities::StepPlan)> {
        capabilities::CHAIN
            .iter()
            .map(|s| (s, capabilities::StepPlan::Run))
            .collect()
    }

    /// Plan as the enrichment lane actually runs today: no retrieval, no
    /// shell.
    fn lane_plan() -> Vec<(&'static capabilities::StepSpec, capabilities::StepPlan)> {
        let grant: std::collections::BTreeSet<String> = capabilities::ENRICH_GRANT
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        capabilities::CHAIN
            .iter()
            .map(|spec| {
                let declared = match spec.name {
                    "MvT research" => "---\nallowed-tools: Read, Write, WebFetch, WebSearch, Bash, Grep, Glob\n---\n",
                    "Generate machine_readable" => {
                        "---\nallowed-tools: Read, Edit, Write, Bash, Grep, Glob\n---\n"
                    }
                    _ => "---\nallowed-tools: Read, Edit, Write, Grep, Glob\n---\n",
                };
                (spec, capabilities::plan_step(spec, &grant, Some(declared)))
            })
            .collect()
    }

    #[test]
    fn usage_is_read_from_the_providers_closing_object() {
        // The shape the claude CLI emits under `--output-format json`.
        let tail = r#"{"type":"result","subtype":"success","is_error":false,
            "duration_ms":812345,"num_turns":42,"result":"done",
            "total_cost_usd":1.2345,
            "usage":{"input_tokens":1200,"output_tokens":34567,
                     "cache_read_input_tokens":980000,"cache_creation_input_tokens":45}}"#;
        let u = AgentUsage::from_stdout_tail(tail).expect("usage");
        assert_eq!(u.input_tokens, 1200);
        assert_eq!(u.output_tokens, 34567);
        assert_eq!(u.cache_read_tokens, 980_000);
        // Money as an integer: 1.2345 dollar is 123450 thousandths of a cent.
        assert_eq!(u.cost_millicents, 123_450);
    }

    #[test]
    fn only_the_last_object_in_the_stream_counts() {
        // The stream carries earlier objects; the accounting is the final one.
        let tail = concat!(
            r#"{"type":"assistant","usage":{"input_tokens":1,"output_tokens":1}}"#,
            "\n",
            r#"{"type":"result","usage":{"input_tokens":99,"output_tokens":7}}"#,
        );
        let u = AgentUsage::from_stdout_tail(tail).expect("usage");
        assert_eq!(u.input_tokens, 99);
        assert_eq!(u.output_tokens, 7);
    }

    #[test]
    fn usage_is_found_in_a_stream_whatever_the_key_order() {
        // stream-json as the CLI writes it: the closing event does not start
        // with `type`, and a stream event with a nested `type` follows it.
        let tail = concat!(
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1}}}"#,
            "\n",
            r#"{"duration_api_ms":5,"type":"result","total_cost_usd":0.01,"usage":{"input_tokens":42,"output_tokens":7}}"#,
            "\n",
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
            "\n",
        );
        let u = AgentUsage::from_stdout_tail(tail).expect("usage");
        assert_eq!(u.input_tokens, 42);
        assert_eq!(u.output_tokens, 7);
        assert_eq!(u.cost_millicents, 1_000);
    }

    #[test]
    fn a_truncated_tail_reports_nothing_rather_than_guessing() {
        // The tail is bounded, so a large stream can cut an object in half.
        // Half a figure is worse than no figure.
        assert!(AgentUsage::from_stdout_tail(r#"put_tokens":1200,"output_"#).is_none());
        assert!(AgentUsage::from_stdout_tail("").is_none());
        assert!(AgentUsage::from_stdout_tail("no json here at all").is_none());
    }

    #[test]
    fn an_object_without_usage_reports_nothing() {
        // A provider that reports no accounting must not read as zero cost.
        assert!(AgentUsage::from_stdout_tail(r#"{"type":"result","result":"ok"}"#).is_none());
    }

    #[test]
    fn a_session_names_each_model_once_in_order() {
        let session = AgentSession::new(SessionReuse::Window);
        assert!(session.models().is_empty());
        session.record(
            "translate",
            SessionAction::Start(session.id()),
            None,
            Some("claude-opus-5-5".into()),
        );
        session.record("schema", SessionAction::Resume(session.id()), None, None);
        session.record(
            "binding",
            SessionAction::Resume(session.id()),
            None,
            Some("claude-opus-5-5".into()),
        );
        session.record(
            "marking",
            SessionAction::Cold,
            None,
            Some("claude-sonnet-5-5".into()),
        );
        assert_eq!(session.models(), ["claude-opus-5-5", "claude-sonnet-5-5"]);
        assert_eq!(session.calls()[1].model, None);
    }

    #[test]
    fn usage_adds_up_over_a_chain() {
        let a = AgentUsage {
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 30,
            cache_write_tokens: 0,
            cost_millicents: 40,
        };
        let b = AgentUsage {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_tokens: 3,
            cache_write_tokens: 0,
            cost_millicents: 4,
        };
        let sum = a.plus(b);
        assert_eq!(sum.input_tokens, 11);
        assert_eq!(sum.output_tokens, 22);
        assert_eq!(sum.cache_read_tokens, 33);
        assert_eq!(sum.cost_millicents, 44);
    }

    #[test]
    fn the_prompt_asks_for_no_source_it_cannot_hand_over() {
        // The measured failure of round 2: the agent was told to search for
        // parliamentary documents without any way to retrieve them, and
        // answered with a kst- citation for a document it never read. The step
        // that asked for it is gone from the chain; what has to stay is the
        // rule, because the pull towards a remembered citation does not need an
        // instruction to invite it.
        let prompt = build_prompt("law.yaml", "/tmp/p.json", &lane_plan(), None, false, false);
        assert!(!prompt.contains("Memorie van Toelichting"));
        assert!(prompt.contains("Cite no source you have not read"));
    }

    #[test]
    fn prompt_names_what_a_degraded_step_may_not_simulate() {
        // law-generate declares Bash for its validate loop; the step still
        // runs, and the prompt must say the loop is not available here and
        // what replaces it.
        let prompt = build_prompt("law.yaml", "/tmp/p.json", &lane_plan(), None, false, false);
        assert!(prompt.contains("law-generate/SKILL.md"));
        assert!(prompt.contains("Bash"));
        assert!(prompt.contains("do not simulate"));
        assert!(prompt.contains("worker validates"));
    }

    #[test]
    fn prompt_points_at_the_context_brief_when_there_is_one() {
        // Requirement 6 of RFC-026: the agent must be told what bears on the
        // article, not left to find it.
        let with = build_prompt("law.yaml", "/tmp/p.json", &lane_plan(), None, false, true);
        assert!(with.contains(context::CONTEXT_BRIEF));
        assert!(with.contains("modify it"));
        assert!(with.contains("not as it reads alone"));

        // And never point at a file that was not written.
        let without = build_prompt("law.yaml", "/tmp/p.json", &lane_plan(), None, false, false);
        assert!(!without.contains(context::CONTEXT_BRIEF));
    }

    #[test]
    fn steps_are_numbered_consecutively_after_an_omission() {
        // A gap in the numbering tells the agent something was withheld and
        // invites it to fill in the blank.
        let prompt = build_prompt("law.yaml", "/tmp/p.json", &lane_plan(), None, false, false);
        assert!(prompt.contains("## Step 1: Generate machine_readable"));
        assert!(prompt.contains("## Step 2: Reverse validation"));
        assert!(prompt.contains("## Step 3: Session report"));
        assert!(!prompt.contains("## Step 4"));
    }

    #[test]
    fn chunk_prompt_restricts_to_the_window() {
        let numbers = vec!["1".to_string(), "2".to_string(), "3a".to_string()];
        let prompt = build_prompt(
            "regulation/nl/wet/test/2025-01-01.yaml",
            "/tmp/repo/.enrichment-progress.json",
            &full_plan(),
            Some(&numbers),
            false,
            false,
        );
        assert!(prompt.contains("Process ONLY these articles"));
        assert!(prompt.contains("1, 2, 3a"));
        assert!(prompt.contains("restricted to the article subset"));
        assert!(prompt.contains("only for the articles you edited"));
        assert!(prompt.contains("chunk_report"));
        assert!(prompt.contains("articles_skipped"));
    }

    fn feedback(gate: Gate) -> Feedback {
        Feedback {
            gate,
            findings: vec!["[accounted] art. 1: the article carries no outcome".to_string()],
        }
    }

    #[test]
    fn schema_version_decides_which_names_exist() {
        assert!(!schema_has_markings("v0.5.6"));
        assert!(!schema_has_markings("v0.4.0"));
        assert!(schema_has_markings("v0.7.0"));
        assert!(schema_has_markings("v0.7.1"));
        assert!(schema_has_markings("v1.0.0"));
        // Unparseable is read as current: sending an agent back to fields the
        // schema dropped is the failure this distinction exists to avoid.
        assert!(schema_has_markings("nonsense"));
    }

    /// The boundary is v0.7.0, not v0.6.0. `schema/v0.6.0/schema.json` defines
    /// `untranslatables` and has no `markings` at all, so telling an agent to
    /// write markings into a v0.6.x law produces a file its own schema rejects:
    /// `machine_readable` is `additionalProperties: false`, and `just validate`
    /// fails on the result.
    ///
    /// Its own test never covered v0.6.0, the one version where the off-by-one
    /// bites, so the mistake sat behind a green suite.
    #[test]
    fn v0_6_x_still_speaks_the_old_vocabulary() {
        assert!(!schema_has_markings("v0.6.0"));
        assert!(!schema_has_markings("v0.6.1"));
    }

    #[test]
    fn vocabulary_is_read_off_the_file() {
        assert_eq!(
            vocabulary_of_yaml(four_article_law()),
            Vocabulary::Markings,
            "the fixture declares v0.7.0"
        );
        assert_eq!(
            vocabulary_of_yaml(&four_article_law().replace(
                "schema-v0.7.0/schema/v0.7.0/schema.json",
                "schema-v0.5.6/schema/v0.5.6/schema.json"
            )),
            Vocabulary::Legacy
        );
        // No `$schema` at all, and not even YAML: both fall to the current
        // vocabulary rather than to the dropped one.
        assert_eq!(vocabulary_of_yaml("articles: []\n"), Vocabulary::Markings);
        assert_eq!(vocabulary_of_yaml("\tnot: [yaml"), Vocabulary::Markings);
    }

    #[test]
    fn feedback_prompt_never_prescribes_a_field_the_schema_forbids() {
        // The deadlock this guards against: a v0.7.0 law fails a gate, the
        // prompt tells the agent to write `norm_gaps`, and the schema gate
        // rejects exactly what the prompt asked for — every round, forever.
        for gate in [Gate::Schema, Gate::Checks, Gate::Marking] {
            let prompt = build_feedback_prompt("law.yaml", &feedback(gate), Vocabulary::Markings);
            assert!(
                !prompt.contains("norm_gap") && !prompt.contains("untranslatable"),
                "{gate:?} prompt names a field v0.7.0 dropped: {prompt}"
            );
        }
    }

    #[test]
    fn feedback_prompt_teaches_the_v0_6_decision_rule() {
        let marking =
            build_feedback_prompt("law.yaml", &feedback(Gate::Marking), Vocabulary::Markings);
        // A value another law produces is an input, not a gap.
        assert!(marking.contains("input with a `source`"));
        // A norm filled in elsewhere is an open term, whoever fills it.
        assert!(marking.contains("`open_term`"));
        assert!(marking.contains("redelijkerwijs"));
        // Only what the format cannot express is a marking, and it says how.
        assert!(marking.contains("`resolution: operation`"));
        assert!(marking.contains("`resolution: model`"));
        // And a marking leaves the rest of the article standing.
        assert!(marking.contains("otherwise worked out"));
        assert!(marking.contains("`target`"));
        // The evidence rule survives the migration.
        assert!(marking.contains("Do not cite what you were not given"));

        let checks =
            build_feedback_prompt("law.yaml", &feedback(Gate::Checks), Vocabulary::Markings);
        assert!(checks.contains("`open_term`"));
        assert!(checks.contains("`marking`"));
        assert!(checks.contains("input with a `source`"));
        // And it says where the answer goes. Round 5's agent wrote a correct,
        // reasoned answer into `.enrichment-result.yaml`; the check reads only
        // the law YAML, so every finding came back word for word and the
        // reasoning was read by nobody.
        assert!(checks.contains(".enrichment-result.yaml"), "{checks}");
        assert!(checks.contains("is not \nan answer") || checks.contains("is not an answer"));

        // The schema gate is the one that may not be answered with a marking;
        // it still has to name the drawer a construct belongs in.
        let schema =
            build_feedback_prompt("law.yaml", &feedback(Gate::Schema), Vocabulary::Markings);
        assert!(schema.contains("`markings`"));
        assert!(schema.contains("`open_terms`"));
    }

    #[test]
    fn feedback_prompt_keeps_the_old_names_for_laws_on_the_old_schema() {
        // v0.5.x laws stay in the corpus and are re-enriched. Naming v0.7.0
        // fields to them is the same deadlock in the other direction.
        let marking =
            build_feedback_prompt("law.yaml", &feedback(Gate::Marking), Vocabulary::Legacy);
        assert!(marking.contains("`untranslatable`"));
        assert!(marking.contains("`norm_gap`"));
        assert!(!marking.contains("`marking`"));
        assert!(marking.contains("Do not cite what you were not given"));

        let checks = build_feedback_prompt("law.yaml", &feedback(Gate::Checks), Vocabulary::Legacy);
        assert!(checks.contains("`untranslatables`"));
        assert!(checks.contains("`norm_gaps`"));

        let schema = build_feedback_prompt("law.yaml", &feedback(Gate::Schema), Vocabulary::Legacy);
        assert!(schema.contains("`untranslatables` with a reason"));
        assert!(!schema.contains("`markings`"));
    }

    #[test]
    fn enrich_payload_chunk_fields_are_transport_only() {
        // Queue payloads never carry the chunk fields: absent when None…
        let bare = EnrichPayload {
            pass: Pass::Translate,
            law_id: "x".into(),
            yaml_path: "regulation/a.yaml".into(),
            provider: None,
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert!(!json.contains("chunk_articles"));
        assert!(!json.contains("skip_mvt"));
        // …and old payload JSON without them still deserializes.
        let old = serde_json::json!({"law_id": "x", "yaml_path": "regulation/a.yaml"});
        let parsed: EnrichPayload = serde_json::from_value(old).unwrap();
        assert!(parsed.chunk_articles.is_none());
        assert!(parsed.skip_mvt.is_none());
    }

    #[test]
    fn test_envelope_chunk_report_roundtrip_and_backcompat() {
        // Old envelopes (without chunk_report) keep parsing.
        let old = "related_legislation:\n  - name: Some Law\n";
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(old).unwrap();
        assert!(envelope.chunk_report.is_none());

        let yaml = r#"
related_legislation:
  - name: Some Law
chunk_report:
  articles_reviewed: ["1", "2", "3"]
  articles_skipped:
    - number: "2"
      reason: definition article
"#;
        let envelope: EnrichmentResultEnvelope = serde_yaml_ng::from_str(yaml).unwrap();
        let report = envelope.chunk_report.expect("chunk_report parses");
        assert_eq!(report.articles_reviewed, vec!["1", "2", "3"]);
        assert_eq!(report.articles_skipped.len(), 1);
        assert_eq!(report.articles_skipped[0].number, "2");
        assert_eq!(report.articles_skipped[0].reason, "definition article");
    }

    /// Chunk-aware fake runner: adds `machine_readable` ONLY to the articles
    /// listed in `payload.chunk_articles` and records every invocation's
    /// window + skip_mvt so the loop contract can be asserted.
    struct FakeChunkRunner {
        calls: std::sync::Mutex<Vec<(Vec<String>, Option<bool>)>>,
        /// Also write a `chunk_report` envelope next to the YAML.
        write_report: bool,
    }

    impl FakeChunkRunner {
        fn new(write_report: bool) -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                write_report,
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmRunner for FakeChunkRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            // A feedback pass carries no window: it asks about the file as
            // it now stands. Only the translating passes are the subject of
            // this test, so record those and let the rest through.
            let Some(chunk) = payload.chunk_articles.clone() else {
                return Ok(());
            };
            self.calls
                .lock()
                .unwrap()
                .push((chunk.clone(), payload.skip_mvt));

            let content = tokio::fs::read_to_string(yaml_abs).await?;
            let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            if let serde_yaml_ng::Value::Mapping(ref mut map) = value {
                if let Some(serde_yaml_ng::Value::Sequence(ref mut articles)) =
                    map.get_mut("articles")
                {
                    for article in articles.iter_mut() {
                        if let serde_yaml_ng::Value::Mapping(ref mut article_map) = article {
                            let number = article_map
                                .get("number")
                                .and_then(|n| n.as_str())
                                .unwrap_or_default()
                                .to_string();
                            if chunk.contains(&number)
                                && !article_map.contains_key("machine_readable")
                            {
                                article_map.insert(
                                    serde_yaml_ng::Value::String("machine_readable".into()),
                                    serde_yaml_ng::Value::Mapping(Default::default()),
                                );
                            }
                        }
                    }
                }
            }
            tokio::fs::write(yaml_abs, serde_yaml_ng::to_string(&value)?).await?;

            if self.write_report {
                let report = format!(
                    "chunk_report:\n  articles_reviewed: [{}]\n",
                    chunk
                        .iter()
                        .map(|n| format!("\"{n}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                tokio::fs::write(enrichment_result_path(yaml_abs), report).await?;
            }
            Ok(())
        }
    }

    fn four_article_law() -> &'static str {
        r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001/2025-01-01
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Article one.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1
  - number: '2'
    text: Article two.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel2
  - number: '3'
    text: Article three.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel3
  - number: '4'
    text: Article four.
    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel4
"#
    }

    /// [`four_article_law`] with the first window (articles 1-2) already
    /// modelled: the shape a window of definition provisions has when the only
    /// thing left to add is a flag.
    fn four_article_law_with_models() -> String {
        four_article_law()
            .replace("#Artikel1\n", "#Artikel1\n    machine_readable: {}\n")
            .replace("#Artikel2\n", "#Artikel2\n    machine_readable: {}\n")
    }

    fn chunk_test_payload(yaml_path: &str) -> EnrichPayload {
        EnrichPayload {
            pass: Pass::Translate,
            law_id: "BWBR0000001".into(),
            yaml_path: yaml_path.into(),
            provider: Some("opencode".into()),
            depth: None,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: None,
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            session: None,
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_chunked_loop_terminates() {
        // 4 articles, 2 per run: the loop must finish in exactly 2 runs, the
        // cursor persisting via .enrichment.yaml between them.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let payload = chunk_test_payload(yaml_path);
        let runner = FakeChunkRunner::new(false);

        // Run 1: articles 1-2, MvT research included, law not complete.
        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        assert_eq!(result.articles_with_machine_readable, 2);
        assert!((result.coverage_score - 0.5).abs() < f64::EPSILON);

        // The hard gate runs again after the two soft ones, because those two
        // write and round 5's marking round left the file unloadable. Without
        // the closing pass nothing looks at the file until the session is
        // already gone.
        let gates: Vec<&str> = result.feedback.iter().map(|g| g.gate.as_str()).collect();
        assert_eq!(
            gates,
            vec!["schema", "checks", "marking", "schema-final"],
            "gate order changed"
        );

        // Cursor persisted on disk for the continuation run.
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(
            &tokio::fs::read_to_string(law_dir.join(".enrichment.yaml"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(meta.enrich_cursor, 2);
        assert_eq!(meta.enrich_cursor_path, yaml_path);

        // Run 2: articles 3-4, MvT research skipped, law complete.
        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 4);
        assert_eq!(result.articles_with_machine_readable, 4);

        // The last window brings the closing pass with it, and the hard gate
        // has to come after it. It used to come before: the reconcile agent
        // then wrote behind the last schema gate, with only `load_law` left,
        // and the model is on documented points wider than the schema. The
        // binding gate sits between the two: after the pass that connects,
        // before the gate that has the last word on the file (#1036).
        let gates: Vec<&str> = result.feedback.iter().map(|g| g.gate.as_str()).collect();
        assert_eq!(
            gates,
            vec![
                "schema",
                "checks",
                "marking",
                "reconcile",
                "binding-final",
                "schema-final"
            ],
            "de harde poort staat achter elke schrijver, ook achter de afrondende pass"
        );

        let calls = runner.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![
                (vec!["1".to_string(), "2".to_string()], Some(false)),
                (vec!["3".to_string(), "4".to_string()], Some(true)),
            ]
        );
    }

    /// Entry 1 of [`four_article_law`], reading the standaardpremie from an
    /// output of this same law. Entry 3 produces it.
    const READS_OWN_OUTPUT: &str = r"    machine_readable:
      endpoint: hoogte
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
            description: Het burgerservicenummer.
        input:
          - name: premie
            type: amount
            description: De standaardpremie.
            source:
              output: standaardpremie
              parameters:
                bsn: $bsn
        output:
          - name: hoogte
            type: amount
            description: De hoogte.
        actions:
          - output: hoogte
            value: $premie
            legal_basis:
              law: Testwet
              bwb_id: BWBR0000001
              article: '1'
              explanation: Het artikel stelt de hoogte gelijk aan de premie.
";

    /// Entry 3 of [`four_article_law`]: what entry 1 reads.
    const PRODUCES_PREMIUM: &str = r"    machine_readable:
      endpoint: standaardpremie
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
            description: Het burgerservicenummer.
        output:
          - name: standaardpremie
            type: amount
            description: De standaardpremie.
        actions:
          - output: standaardpremie
            value: 1000
            legal_basis:
              law: Testwet
              bwb_id: BWBR0000001
              article: '3'
              explanation: Het artikel stelt de standaardpremie vast.
";

    /// A translator that writes a fixed model per entry of its window, and a
    /// fixed answer to the binding gate, recording which passes it was given.
    struct ScriptedRunner {
        models: Vec<(&'static str, String)>,
        /// Text replacement applied when the binding gate asks for a repair.
        binding_fix: Option<(&'static str, &'static str)>,
        /// Text replacements applied when the schema gate asks for a repair.
        schema_fix: Vec<(&'static str, &'static str)>,
        passes: std::sync::Mutex<Vec<String>>,
        /// The ceiling each call ran under, in call order.
        ceilings: std::sync::Mutex<Vec<Duration>>,
    }

    impl ScriptedRunner {
        fn new(models: Vec<(&'static str, String)>) -> Self {
            Self {
                models,
                binding_fix: None,
                schema_fix: Vec::new(),
                passes: std::sync::Mutex::new(Vec::new()),
                ceilings: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmRunner for ScriptedRunner {
        async fn run(
            &self,
            payload: &EnrichPayload,
            yaml_abs: &Path,
            _repo_path: &Path,
            config: &EnrichConfig,
        ) -> Result<()> {
            self.ceilings.lock().unwrap().push(config.timeout);
            let mut text = tokio::fs::read_to_string(yaml_abs).await?;
            match &payload.pass {
                Pass::Translate => {
                    self.passes.lock().unwrap().push("translate".into());
                    let window = payload
                        .chunk_articles
                        .clone()
                        .unwrap_or_else(|| ["1", "2", "3", "4"].map(String::from).to_vec());
                    for number in window {
                        let model = self
                            .models
                            .iter()
                            .find(|(n, _)| *n == number)
                            .map(|(_, m)| m.clone())
                            .unwrap_or_else(|| "    machine_readable: {}\n".to_string());
                        let anchor = format!("#Artikel{number}\n");
                        text = text.replacen(&anchor, &format!("{anchor}{model}"), 1);
                    }
                }
                Pass::Feedback(f) => {
                    self.passes.lock().unwrap().push(f.gate.label().into());
                    if let (Gate::Binding, Some((from, to))) = (f.gate, self.binding_fix) {
                        text = text.replace(from, to);
                    }
                    if f.gate == Gate::Schema {
                        for (from, to) in &self.schema_fix {
                            text = text.replace(from, to);
                        }
                    }
                }
            }
            tokio::fs::write(yaml_abs, text).await?;
            Ok(())
        }
    }

    /// The forward reference the reconcile gate exists for, end to end: the
    /// first window reads an output that only the second window writes. That
    /// is soft while the walk is under way, and resolved once it is over, so
    /// neither run fails and the binding gate asks nobody anything (#1036).
    #[tokio::test]
    async fn a_reference_a_later_window_resolves_does_not_fail_the_walk() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let payload = chunk_test_payload(yaml_path);
        let runner = ScriptedRunner::new(vec![
            ("1", READS_OWN_OUTPUT.to_string()),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);

        // Window 1: entry 1 reads an output nobody produces yet.
        let (first, _) = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .expect("a forward reference inside the walk is not fatal");
        assert!(!first.law_complete);
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &tokio::fs::read_to_string(dir.path().join(yaml_path))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            crate::enrich_v2::checks::dangling_bindings(&doc).len(),
            1,
            "na het eerste venster wijst entry 1 nog naar niets"
        );
        assert!(
            !first.feedback.iter().any(|g| g.gate == "binding-final"),
            "een venster halverwege de wandeling krijgt de bindingspoort niet"
        );

        // Window 2: entry 3 produces it, and the law is complete.
        let (second, _) = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .expect("the reference resolves once the walk is over");
        assert!(second.law_complete);
        let binding = second
            .feedback
            .iter()
            .find(|g| g.gate == "binding-final")
            .expect("the binding gate runs after the last window");
        assert_eq!(binding.findings_initial, 0);
        assert!(binding.rounds.is_empty());
        assert!(!runner.passes.lock().unwrap().iter().any(|p| p == "binding"));
    }

    /// The defect itself: a law that is schema-valid and reads a name nothing
    /// defines. It used to be recorded as a soft finding and committed; now
    /// the agent gets a repair round, and when that does not help the job
    /// fails, which means the worker commits nothing (#1036).
    #[tokio::test]
    async fn a_reference_to_nothing_fails_the_job_after_a_repair_round() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        let payload = chunk_test_payload(yaml_path);
        let runner = ScriptedRunner::new(vec![
            (
                "1",
                READS_OWN_OUTPUT.replace("value: $premie", "value: $onbekend"),
            ),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);

        let err = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .expect_err("a law that cannot run must not be committed");
        let message = err.to_string();
        assert!(
            message.contains("reference(s) to nothing in this law after 1 feedback round(s)")
                && message.contains("$onbekend"),
            "{message}"
        );
        // Retryable like the schema gate, not an exhaust-in-one-step failure.
        assert!(!message.to_ascii_lowercase().contains("yaml error"));
        assert!(!message.contains("no machine_readable sections"));
        let passes = runner.passes.lock().unwrap().clone();
        assert!(
            passes.iter().any(|p| p == "binding"),
            "de agent krijgt eerst een herstelronde: {passes:?}"
        );
    }

    /// The other half of the same gate: a repair that makes the name resolve
    /// lets the law through, and the schema gate after it still gets the
    /// last word.
    #[tokio::test]
    async fn a_repaired_reference_lets_the_law_through() {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        let payload = chunk_test_payload(yaml_path);
        let mut runner = ScriptedRunner::new(vec![
            (
                "1",
                READS_OWN_OUTPUT.replace("value: $premie", "value: $onbekend"),
            ),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);
        runner.binding_fix = Some(("value: $onbekend", "value: $premie"));

        let (result, _) = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .expect("a repaired reference is no defect");
        let binding = result
            .feedback
            .iter()
            .find(|g| g.gate == "binding-final")
            .unwrap();
        assert_eq!(binding.findings_initial, 1);
        assert_eq!(binding.findings_final, 0);
        let gates: Vec<&str> = result.feedback.iter().map(|g| g.gate.as_str()).collect();
        assert_eq!(gates.last(), Some(&"schema-final"));
    }

    /// Walk [`four_article_law`] in two windows with a dangling reference in
    /// entry 1, so the binding repair of the last window writes into an entry
    /// the last window does not own.
    async fn walk_with_a_repair_outside_the_last_window(
        runner: &ScriptedRunner,
    ) -> Result<(EnrichResult, Vec<PathBuf>)> {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let payload = chunk_test_payload(yaml_path);
        execute_enrich_with_runner(&payload, dir.path(), &config, "", runner)
            .await
            .expect("window 1: a dangling reference mid-walk is soft");
        execute_enrich_with_runner(&payload, dir.path(), &config, "", runner).await
    }

    fn dangling_in_entry_one() -> ScriptedRunner {
        let mut runner = ScriptedRunner::new(vec![
            (
                "1",
                READS_OWN_OUTPUT.replace("value: $premie", "value: $onbekend"),
            ),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);
        // The repair connects the name and breaks the schema of entry 1.
        runner.binding_fix = Some((
            "value: $onbekend",
            "value: $premie\n            onbekend_veld: true",
        ));
        runner
    }

    /// The final schema gate judges the entries the binding repair wrote, not
    /// only the last window: entry 1 belongs to the first window, and without
    /// the fold its schema error would have been committed unseen.
    #[tokio::test]
    async fn the_final_schema_gate_sees_what_the_binding_repair_wrote() {
        let mut runner = dangling_in_entry_one();
        runner.schema_fix = vec![("\n            onbekend_veld: true", "")];
        let (result, _) = walk_with_a_repair_outside_the_last_window(&runner)
            .await
            .expect("repaired twice, the law goes through");
        let schema_final = result
            .feedback
            .iter()
            .find(|g| g.gate == "schema-final")
            .expect("the repair wrote, so the schema gate runs");
        assert_eq!(
            schema_final.findings_initial, 1,
            "de schemafout in entry 1 valt buiten het laatste venster en moet toch gezien worden"
        );
        assert_eq!(schema_final.findings_final, 0);
    }

    /// The schema repair is the last writer, so what it leaves is checked for
    /// dangling references once more, without another agent round.
    #[tokio::test]
    async fn a_schema_repair_that_breaks_a_reference_fails_the_job() {
        let mut runner = dangling_in_entry_one();
        runner.schema_fix = vec![
            ("\n            onbekend_veld: true", ""),
            ("value: $premie", "value: $weg"),
        ];
        let err = walk_with_a_repair_outside_the_last_window(&runner)
            .await
            .expect_err("the schema repair left a reference to nothing");
        assert!(
            err.to_string()
                .contains("after the final schema repair: [binding] art. 1: $weg"),
            "{err}"
        );
    }

    /// A law directory with [`four_article_law`] in it, and the config and
    /// payload of a two-window walk over it.
    async fn two_window_walk() -> (tempfile::TempDir, EnrichConfig, EnrichPayload) {
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        (dir, config, chunk_test_payload(yaml_path))
    }

    fn reconcile_only(config: &EnrichConfig) -> EnrichConfig {
        let mut config = config.clone();
        config.steps = RunSteps {
            window: false,
            reconcile: true,
        };
        config
    }

    /// `--steps reconcile` over a law whose walk is over is how a law enriched
    /// before this gate existed gets judged. It translates nothing, so only
    /// the binding repair writes, and the schema gate still gets the last
    /// word over what that repair wrote.
    #[tokio::test]
    async fn a_reconcile_only_run_on_a_finished_law_gets_the_binding_gate() {
        const BROKEN: &str = "\n            onbekend_veld: true";
        let (dir, config, payload) = two_window_walk().await;
        let mut runner = ScriptedRunner::new(vec![
            ("1", READS_OWN_OUTPUT.to_string()),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);
        runner.binding_fix = Some((
            "value: $onbekend",
            "value: $premie\n            onbekend_veld: true",
        ));
        runner.schema_fix = vec![(BROKEN, "")];
        for _ in 0..2 {
            execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
                .await
                .unwrap();
        }
        // What an older run left behind: a name nothing defines.
        let path = dir.path().join(&payload.yaml_path);
        let text = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::write(&path, text.replace("value: $premie", "value: $onbekend"))
            .await
            .unwrap();

        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &reconcile_only(&config), "", &runner)
                .await
                .expect("repaired, the law goes through");
        let gate = |name: &str| result.feedback.iter().find(|g| g.gate == name);
        let binding = gate("binding-final").expect("the walk is over, so the gate runs");
        assert_eq!((binding.findings_initial, binding.findings_final), (1, 0));
        let schema =
            gate("schema-final").expect("the binding repair wrote, so the schema gate runs");
        assert_eq!((schema.findings_initial, schema.findings_final), (1, 0));
    }

    /// The same run halfway through the walk leaves a forward reference
    /// alone: entry 3, which produces what entry 1 reads, has not been
    /// translated yet.
    #[tokio::test]
    async fn a_reconcile_only_run_halfway_the_walk_does_not_get_the_binding_gate() {
        let (dir, config, payload) = two_window_walk().await;
        let runner = ScriptedRunner::new(vec![
            ("1", READS_OWN_OUTPUT.to_string()),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);
        execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .unwrap();

        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &reconcile_only(&config), "", &runner)
                .await
                .expect("a forward reference mid-walk is not fatal");
        assert!(!result.feedback.iter().any(|g| g.gate == "binding-final"));
    }

    /// A layer walk decides completion by layers, not by entries. The run
    /// that walks the last layer is the one that ends the walk, and it gets
    /// the gate.
    #[tokio::test]
    async fn the_run_that_walks_the_last_layer_gets_the_binding_gate() {
        let (dir, mut config, payload) = two_window_walk().await;
        config.window_mode = WindowMode::Layer;
        let runner = ScriptedRunner::new(vec![
            ("1", READS_OWN_OUTPUT.to_string()),
            ("3", PRODUCES_PREMIUM.to_string()),
        ]);
        let mut runs = Vec::new();
        for _ in 0..8 {
            let (result, _) =
                execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
                    .await
                    .unwrap();
            let done = result.law_complete;
            runs.push(result);
            if done {
                break;
            }
        }
        let last = runs.last().unwrap();
        assert!(last.law_complete, "the layer walk ends");
        assert!(last.feedback.iter().any(|g| g.gate == "binding-final"));
        assert!(runs[..runs.len() - 1]
            .iter()
            .all(|r| !r.feedback.iter().any(|g| g.gate == "binding-final")));
    }

    /// The ceiling the worker divides the job budget by is the number of agent
    /// calls a run really makes when every gate has something to say, with the
    /// default one round per gate. Measured by driving a run through every
    /// gate: a translation that leaves a schema error, a checks and a marking
    /// question, a reconcile lead and a dangling reference, and a binding
    /// repair that breaks the schema again for the final schema gate.
    #[tokio::test]
    async fn the_agent_call_ceiling_is_what_a_run_through_every_gate_makes() {
        const BROKEN: &str = "\n            onbekend_veld: true";
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        assert_eq!(config.feedback_rounds, FeedbackRounds::default());
        config.timeout = Duration::from_secs(1290);
        config.feedback_timeout = Duration::from_secs(430);
        let payload = chunk_test_payload(yaml_path);
        // Entry 1 reads a name nothing defines, and leaves open a term that
        // entry 3 produces (a reconcile lead). Entry 3 carries a schema error
        // that the first schema round repairs.
        let reads = READS_OWN_OUTPUT
            .replace("value: $premie", "value: $onbekend")
            .replace(
                "      execution:\n",
                "      open_terms:\n        - id: standaardpremie\n          type: amount\n      \
                 execution:\n",
            );
        let produces = PRODUCES_PREMIUM.replace(
            "            value: 1000",
            &format!("            value: 1000{BROKEN}"),
        );
        let mut runner = ScriptedRunner::new(vec![("1", reads), ("3", produces)]);
        runner.binding_fix = Some((
            "value: $onbekend",
            "value: $premie\n            onbekend_veld: true",
        ));
        runner.schema_fix = vec![(BROKEN, "")];

        let (result, _) = execute_enrich_with_runner(&payload, dir.path(), &config, "", &runner)
            .await
            .expect("every gate is answered");
        let asked: Vec<(String, usize)> = result
            .feedback
            .iter()
            .map(|g| (g.gate.clone(), g.rounds.len()))
            .collect();
        assert_eq!(
            asked,
            vec![
                ("schema".to_string(), 1),
                ("checks".to_string(), 1),
                ("marking".to_string(), 1),
                ("reconcile".to_string(), 1),
                ("binding-final".to_string(), 1),
                ("schema-final".to_string(), 1),
            ],
            "elke poort moet een ronde draaien, anders meet deze test het plafond niet"
        );
        let calls = runner.passes.lock().unwrap().len();
        assert_eq!(calls as u32, MAX_AGENT_CALLS_PER_RUN);
        // The translation runs under its own ceiling, every round under the
        // smaller one.
        let ceilings = runner.ceilings.lock().unwrap().clone();
        let mut expected = vec![Duration::from_secs(1290)];
        expected.extend(std::iter::repeat_n(Duration::from_secs(430), calls - 1));
        assert_eq!(ceilings, expected);
    }

    #[test]
    fn changed_entries_names_what_a_rewrite_touched() {
        let before = four_article_law();
        let after = before.replace("#Artikel3\n", "#Artikel3\n    machine_readable: {}\n");
        assert_eq!(changed_entries(before, &after), vec!["3".to_string()]);
        assert!(changed_entries(before, before).is_empty());
        assert!(changed_entries(before, "articles: [").is_empty());
    }

    #[tokio::test]
    async fn a_named_entry_is_the_only_one_enriched_and_the_cursor_stands_still() {
        // Targeted work: article 4 is repaired, the walk through the document
        // has not begun, and it must not look as if it had.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        config.target_articles = vec!["4".into()];
        let payload = chunk_test_payload(yaml_path);
        let runner = FakeChunkRunner::new(false);

        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();

        // Exactly the named entry, and the MvT research is not redone.
        assert_eq!(
            *runner.calls.lock().unwrap(),
            vec![(vec!["4".to_string()], Some(true))]
        );
        assert_eq!(result.articles_with_machine_readable, 1);
        // The cursor did not move: this was a repair, not progress.
        assert_eq!(result.enrich_cursor, 0);
        assert!(!result.law_complete);
        let meta: EnrichmentMetadata = serde_yaml_ng::from_str(
            &tokio::fs::read_to_string(law_dir.join(".enrichment.yaml"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(meta.enrich_cursor, 0);

        // And the ordinary walk still starts at the beginning, which is the
        // termination property the cursor mode may not lose.
        config.target_articles = Vec::new();
        let (result, _) =
            execute_enrich_with_runner(&payload, dir.path(), &config, "sha1", &runner)
                .await
                .unwrap();
        assert_eq!(result.enrich_cursor, 2);
        assert_eq!(
            runner.calls.lock().unwrap()[1],
            (vec!["1".to_string(), "2".to_string()], Some(false))
        );
    }

    #[tokio::test]
    async fn a_named_entry_the_law_does_not_have_fails_the_run() {
        // Silence is the wrong answer: whoever names an entry that is not
        // there has a mistake in their query, and a run that enriched nothing
        // looks exactly like a run that found nothing to do.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.target_articles = vec!["2.1.i".into()];
        let runner = FakeChunkRunner::new(false);

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "sha1",
            &runner,
        )
        .await
        .expect_err("an entry that does not exist must fail the run");
        assert!(
            err.to_string().contains("2.1.i"),
            "the message must name the entry: {err}"
        );
        assert!(
            runner.calls.lock().unwrap().is_empty(),
            "no agent runs on a target that is not there"
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_noop_with_report_succeeds() {
        // A chunk that adds no machine_readable but writes a chunk_report is
        // legitimate progress: the cursor advances and the run succeeds.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        /// Writes only a chunk_report — no machine_readable at all.
        struct ReportOnlyRunner;
        #[async_trait::async_trait]
        impl LlmRunner for ReportOnlyRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                tokio::fs::write(
                    enrichment_result_path(yaml_abs),
                    "chunk_report:\n  articles_reviewed: [\"1\", \"2\"]\n  articles_skipped:\n    - number: \"1\"\n      reason: transitional law\n",
                )
                .await?;
                Ok(())
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &ReportOnlyRunner,
        )
        .await
        .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        assert_eq!(result.articles_with_machine_readable, 0);
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_noop_without_report_fails_retryable() {
        // No machine_readable, no chunk_report, no untranslatables: the chunk
        // fails — with a message that must NOT be classified as a
        // deterministic content failure (the worker pins that classification).
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &NoopLlmRunner,
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(CHUNK_NO_OUTPUT_MARKER), "got: {msg}");
        assert!(!msg.contains("no machine_readable sections"), "got: {msg}");
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_marking_only_is_progress() {
        // The window is two articles that already carry a model — definition
        // provisions, typically — and the right answer is a marking on one of
        // them: nothing new gets a machine_readable section and no
        // untranslatable is written. The guard must read that as reviewed
        // work. Counting only untranslatables failed this window and sent a
        // correctly-handled chunk back around the retry loop.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law_with_models())
            .await
            .unwrap();

        /// Adds one marking to the first already-modelled article and nothing
        /// else: no new sections, no chunk_report, no untranslatables.
        struct MarkingOnlyRunner;
        #[async_trait::async_trait]
        impl LlmRunner for MarkingOnlyRunner {
            async fn run(
                &self,
                payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                if !matches!(payload.pass, Pass::Translate) {
                    return Ok(());
                }
                // The prompt names this path to the agent, so a run that
                // handed over an empty one would send it looking for nothing.
                assert_eq!(
                    payload.yaml_path, "regulation/nl/wet/test_law/2025-01-01.yaml",
                    "the runner must be given the normalized law path"
                );
                let content = tokio::fs::read_to_string(yaml_abs).await?;
                let updated = content.replacen(
                    "    machine_readable: {}\n",
                    "    machine_readable:\n      \
                     markings:\n        \
                     - about: elke persoon die met de aanvrager samenwoont\n          \
                     reason: het model kent alleen regels over een waarde, niet over een \
                     verzameling personen\n          \
                     resolution: model\n          \
                     resolved_by: kwantificatie over personen\n          \
                     target: []\n          \
                     legal_text_excerpt: Article one.\n",
                    1,
                );
                assert_ne!(updated, content, "fixture must contain a bare model");
                tokio::fs::write(yaml_abs, updated).await?;
                Ok(())
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &MarkingOnlyRunner,
        )
        .await
        .expect("a window whose only output is a marking is reviewed work");

        // The cursor advanced past the reviewed window…
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        // …without a single new machine_readable section or untranslatable,
        // so the marking is the only thing that can have counted.
        assert_eq!(result.articles_with_machine_readable, 2);
        assert!((result.coverage_score - 0.0).abs() < f64::EPSILON);
        assert!(result.untranslatables.is_empty());
        assert_eq!(result.markings.len(), 1);
        assert_eq!(result.markings[0].article, "1");
        assert_eq!(result.markings[0].resolution, "model");
        assert!(result.markings[0].target.is_empty());
    }

    #[test]
    fn an_article_takes_every_entry_the_harvest_hung_under_it() {
        // The Zorgverzekeringswet splits article 69 into 69.1 .. 69.17 with
        // sub-items. Asking for the article has to take all of them, or one
        // article costs 22 sessions.
        let entries: Vec<String> = ["68b", "68b.5", "69", "69.1", "69.4.a", "69.17", "690", "7"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            super::entries_of(&entries, "69"),
            vec!["69", "69.1", "69.4.a", "69.17"]
        );
        // On the separator, not the prefix: 690 is a different article.
        assert!(!super::entries_of(&entries, "69").contains(&"690".to_string()));
        // An entry number still works, and takes only what hangs under it.
        assert_eq!(super::entries_of(&entries, "68b.5"), vec!["68b.5"]);
    }

    #[test]
    fn window_progress_counts_both_gap_channels() {
        // Same window, one marking versus one untranslatable: the guard reads
        // one figure and both channels feed it.
        let with_marking: ArticleBasedLaw =
            serde_yaml_ng::from_str(&four_article_law_with_models().replace(
                "  - number: '1'\n    text: Article one.\n    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1\n    machine_readable: {}\n",
                "  - number: '1'\n    text: Article one.\n    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1\n    machine_readable:\n      markings:\n        - about: iets\n          reason: de motor kent geen wettelijke afronding op eurocenten\n          resolution: operation\n          target: []\n          legal_text_excerpt: Article one.\n",
            ))
            .unwrap();
        let window = vec!["1".to_string(), "2".to_string()];
        assert_eq!(window_progress_stats(&with_marking, &window), (2, 1));

        let with_untranslatable: ArticleBasedLaw =
            serde_yaml_ng::from_str(&four_article_law_with_models().replace(
                "  - number: '1'\n    text: Article one.\n    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1\n    machine_readable: {}\n",
                "  - number: '1'\n    text: Article one.\n    url: https://wetten.overheid.nl/BWBR0000001/2025-01-01#Artikel1\n    machine_readable:\n      untranslatables:\n        - construct: iets\n          reason: omdat\n",
            ))
            .unwrap();
        assert_eq!(window_progress_stats(&with_untranslatable, &window), (2, 1));

        // And a window nobody touched still counts as nothing.
        let untouched: ArticleBasedLaw =
            serde_yaml_ng::from_str(&four_article_law_with_models()).unwrap();
        assert_eq!(window_progress_stats(&untouched, &window), (2, 0));
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_out_of_window_edit_is_no_progress() {
        // A run that adds machine_readable ONLY to an article outside its
        // assigned window (and writes no report) has not reviewed the window:
        // the document-wide count rose, but the guard must still fail
        // retryable instead of advancing the cursor past an unreviewed window.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        // Window is articles 1-2; this runner enriches article 3 instead.
        struct OutOfWindowRunner;
        #[async_trait::async_trait]
        impl LlmRunner for OutOfWindowRunner {
            async fn run(
                &self,
                payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                // Only the translating pass edits. A feedback pass that
                // repeated this edit would write the key twice.
                if !matches!(payload.pass, Pass::Translate) {
                    return Ok(());
                }
                let content = tokio::fs::read_to_string(yaml_abs).await?;
                let updated = content.replace(
                    "  - number: '3'\n    text: Article three.",
                    "  - number: '3'\n    text: Article three.\n    machine_readable: {}",
                );
                tokio::fs::write(yaml_abs, updated).await?;
                Ok(())
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &OutOfWindowRunner,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains(CHUNK_NO_OUTPUT_MARKER),
            "out-of-window edit must not count as window progress: {err}"
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_empty_or_unrelated_report_is_no_proof() {
        // A bare `chunk_report: {}` — or one naming only articles outside the
        // window — must not count as proof-of-review: presence alone would let
        // a do-nothing run advance the cursor past an unreviewed window (and
        // eventually mark the law enriched with silent gaps).
        /// Writes a fixed chunk_report body, nothing else.
        struct FixedReportRunner(&'static str);
        #[async_trait::async_trait]
        impl LlmRunner for FixedReportRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                tokio::fs::write(enrichment_result_path(yaml_abs), self.0).await?;
                Ok(())
            }
        }

        for report in [
            "chunk_report: {}\n",
            // Articles 3-4 are outside the first window (articles 1-2).
            "chunk_report:\n  articles_reviewed: [\"3\", \"4\"]\n",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let law_dir = dir.path().join("regulation/nl/wet/test_law");
            tokio::fs::create_dir_all(&law_dir).await.unwrap();
            let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
            tokio::fs::write(dir.path().join(yaml_path), four_article_law())
                .await
                .unwrap();

            let mut config = test_config(LlmProvider::OpenCode {
                path: "fake".into(),
                model: None,
            });
            config.max_articles_per_run = 2;

            let err = execute_enrich_with_runner(
                &chunk_test_payload(yaml_path),
                dir.path(),
                &config,
                "",
                &FixedReportRunner(report),
            )
            .await
            .unwrap_err();
            assert!(
                err.to_string().contains(CHUNK_NO_OUTPUT_MARKER),
                "report {report:?} must not count as proof: {err}"
            );
        }
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_stale_report_is_no_proof_of_review() {
        // The envelope is committed to the enrich branch as provenance, so a
        // continuation chunk's checkout still contains the PREVIOUS chunk's
        // chunk_report. A run that produces nothing must not pass the no-op
        // guard on that stale report — the worker strips it pre-run, keeping
        // the rest of the envelope (related_legislation) intact.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        // Cursor after chunk 1 (articles 1-2) …
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            format!(
                "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 0\nenrich_cursor: 2\nenrich_cursor_path: {yaml_path}\n"
            ),
        )
        .await
        .unwrap();
        // …and chunk 1's committed envelope, report included.
        tokio::fs::write(
            law_dir.join(".enrichment-result.yaml"),
            "related_legislation:\n  - name: Some Law\n    bwb_id: BWBR0037841\nchunk_report:\n  articles_reviewed: [\"1\", \"2\"]\n",
        )
        .await
        .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let err = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &NoopLlmRunner,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains(CHUNK_NO_OUTPUT_MARKER),
            "stale chunk_report must not count as this session's proof: {err}"
        );

        // The stale report was stripped; the rest of the envelope survived.
        let envelope = read_enrichment_result_envelope(&dir.path().join(yaml_path)).await;
        assert!(envelope.chunk_report.is_none());
        assert_eq!(envelope.related_legislation.len(), 1);
        assert_eq!(
            envelope.related_legislation[0].bwb_id.as_deref(),
            Some("BWBR0037841")
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_cursor_at_end_completes_without_llm() {
        // A valid cursor at the end of the document (loop already finished)
        // completes trivially: no LLM invocation, law_complete = true.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        // Pre-existing metadata with the cursor at the end.
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            format!(
                "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 4\nenrich_cursor: 4\nenrich_cursor_path: {yaml_path}\n"
            ),
        )
        .await
        .unwrap();

        /// Panics when invoked: the empty window must never reach the LLM.
        struct PanickingRunner;
        #[async_trait::async_trait]
        impl LlmRunner for PanickingRunner {
            async fn run(
                &self,
                _payload: &EnrichPayload,
                _yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                panic!("LLM must not run for an empty chunk window");
            }
        }

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &PanickingRunner,
        )
        .await
        .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 4);
    }

    #[tokio::test]
    async fn test_execute_enrich_chunk_cursor_resets_for_new_version() {
        // Metadata recorded for another yaml path (older law version): the
        // cursor must reset to 0 and MvT research must run again.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2026-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();
        tokio::fs::write(
            law_dir.join(".enrichment.yaml"),
            "law_id: BWBR0000001\ntimestamp: '2026-01-01T00:00:00Z'\nprovider: opencode\nmodel: m\nprompt_hash: p\ncode_commit: c\ncoverage_score: 1.0\narticles_total: 4\narticles_with_machine_readable: 4\nenrich_cursor: 2\nenrich_cursor_path: regulation/nl/wet/test_law/2025-01-01.yaml\n",
        )
        .await
        .unwrap();

        let mut config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        config.max_articles_per_run = 2;
        let runner = FakeChunkRunner::new(false);

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &runner,
        )
        .await
        .unwrap();
        assert!(!result.law_complete);
        assert_eq!(result.enrich_cursor, 2);
        let calls = runner.calls.lock().unwrap();
        // Reset to the start: first window, MvT research NOT skipped.
        assert_eq!(
            *calls,
            vec![(vec!["1".to_string(), "2".to_string()], Some(false))]
        );
    }

    #[tokio::test]
    async fn test_execute_enrich_whole_law_has_no_chunk_fields() {
        // N=0: the runner receives a payload without chunk fields, so
        // ProcessLlmRunner builds the byte-identical whole-law prompt.
        let dir = tempfile::tempdir().unwrap();
        let law_dir = dir.path().join("regulation/nl/wet/test_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        let yaml_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
        tokio::fs::write(dir.path().join(yaml_path), four_article_law())
            .await
            .unwrap();

        struct AssertWholeLawRunner;
        #[async_trait::async_trait]
        impl LlmRunner for AssertWholeLawRunner {
            async fn run(
                &self,
                payload: &EnrichPayload,
                yaml_abs: &Path,
                _repo_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                assert!(payload.chunk_articles.is_none());
                assert!(payload.skip_mvt.is_none());
                // Enrich everything so the zero-coverage guard passes.
                FakeLlmRunner
                    .run(payload, yaml_abs, _repo_path, _config)
                    .await
            }
        }

        let config = test_config(LlmProvider::OpenCode {
            path: "fake".into(),
            model: None,
        });
        assert_eq!(config.max_articles_per_run, 0);

        let (result, _) = execute_enrich_with_runner(
            &chunk_test_payload(yaml_path),
            dir.path(),
            &config,
            "",
            &AssertWholeLawRunner,
        )
        .await
        .unwrap();
        assert!(result.law_complete);
        assert_eq!(result.enrich_cursor, 0);
    }
}

/// `run_llm_subprocess` against a shell script standing in for the provider's
/// CLI. The script ignores its arguments and prints what a real run would.
#[cfg(all(test, unix))]
mod subprocess_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A fake claude CLI running `body`, in its own temp dir.
    #[allow(clippy::unwrap_used)] // test setup; a failure here is the failure
    fn fake_claude(body: &str) -> (tempfile::TempDir, LlmProvider) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let provider = LlmProvider::Claude { path, model: None };
        (dir, provider)
    }

    async fn run(
        provider: &LlmProvider,
        cwd: &Path,
        timeout: Duration,
        idle: Option<Duration>,
    ) -> (Result<AgentRun>, EnrichConfig) {
        let mut config = EnrichConfig::for_test(provider.clone());
        config.timeout = timeout;
        config.idle_timeout = idle;
        config.activity.begin_call("translate");
        let out = run_llm_subprocess(
            provider,
            "prompt",
            None,
            cwd,
            &config,
            ToolPolicy {
                allow_bash: false,
                deny: &[],
            },
            SessionAction::Cold,
        )
        .await;
        (out, config)
    }

    const TOOL_EVENT: &str = r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Read","input":{"file_path":"laws/x/law.yaml"}}]}}"#;

    #[tokio::test]
    async fn reads_events_and_usage_from_the_stream() {
        // The closing event as the CLI writes it: `type` is not the first
        // key, and an event can follow it.
        let (dir, provider) = fake_claude(&format!(
            "echo '{{\"type\":\"system\",\"subtype\":\"init\",\"model\":\"claude-opus-5-5\"}}'\n\
             echo '{TOOL_EVENT}'\n\
             echo '{{\"type\":\"stream_event\",\"event\":{{}}}}'\n\
             echo '{{\"duration_api_ms\":812,\"is_error\":false,\"type\":\"result\",\"total_cost_usd\":0.5,\"usage\":{{\"input_tokens\":10,\"output_tokens\":20}}}}'\n\
             echo '{{\"type\":\"stream_event\",\"event\":{{\"type\":\"message_stop\"}}}}'"
        ));
        let (out, config) = run(&provider, dir.path(), Duration::from_secs(20), None).await;
        let run = out.expect("run succeeds");
        assert_eq!(run.model.as_deref(), Some("claude-opus-5-5"));
        let usage = run.usage.expect("usage reported");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.cost_millicents, 50_000);
        let s = config.activity.snapshot();
        assert_eq!(s["turns"], 1);
        assert_eq!(s["tool_calls"], 1);
        assert_eq!(s["last_tool"], "Read laws/x/law.yaml");
    }

    #[tokio::test]
    async fn a_silent_run_is_stopped_as_stalled() {
        let (dir, provider) = fake_claude(&format!("echo '{TOOL_EVENT}'\nsleep 30"));
        let started = std::time::Instant::now();
        let (out, _) = run(
            &provider,
            dir.path(),
            Duration::from_secs(20),
            Some(Duration::from_millis(400)),
        )
        .await;
        let err = out.expect_err("a silent run fails").to_string();
        assert!(err.contains("stalled: no output for"), "{err}");
        assert!(err.contains("recent tools: Read laws/x/law.yaml"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "stopped long before the timeout"
        );
    }

    #[tokio::test]
    async fn a_slow_run_that_keeps_writing_is_not_stalled() {
        // Longer in total than the stall limit, but never silent for that long.
        let (dir, provider) = fake_claude(
            "for i in 1 2 3 4 5 6 7 8; do echo '{\"type\":\"stream_event\"}'; sleep 0.1; done",
        );
        let (out, _) = run(
            &provider,
            dir.path(),
            Duration::from_secs(20),
            Some(Duration::from_millis(500)),
        )
        .await;
        out.expect("a run that keeps writing completes");
    }

    #[tokio::test]
    async fn a_timeout_says_what_the_agent_was_doing() {
        let (dir, provider) = fake_claude(&format!("echo '{TOOL_EVENT}'\nsleep 30"));
        let (out, _) = run(&provider, dir.path(), Duration::from_millis(500), None).await;
        let err = out.expect_err("the run times out").to_string();
        assert!(err.contains("timed out after"), "{err}");
        assert!(
            err.contains("agent activity: 1 turns, 1 tool calls"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn a_failed_run_says_what_the_cli_reported() {
        // A failed API call: one synthetic assistant message, an error
        // result, exit 1, and nothing on stderr.
        let (dir, provider) = fake_claude(
            "echo '{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"text\",\"text\":\"API Error: 429\"}]}}'\n\
             echo '{\"duration_ms\":240000,\"is_error\":true,\"type\":\"result\",\"subtype\":\"success\",\"result\":\"API Error: 429 rate_limit_error\"}'\n\
             exit 1",
        );
        let (out, _) = run(&provider, dir.path(), Duration::from_secs(20), None).await;
        let err = out.expect_err("the run fails").to_string();
        assert!(
            err.contains("exited with exit status: 1: API Error: 429 rate_limit_error; agent activity: 1 turns, 0 tool calls"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn opencode_is_not_stopped_for_silence() {
        // opencode writes an event per finished part, not per token, so a
        // quiet stretch is normal there and the check does not apply.
        let (dir, claude) = fake_claude("sleep 1");
        let LlmProvider::Claude { path, .. } = claude else {
            unreachable!("fake_claude builds a claude provider")
        };
        let provider = LlmProvider::OpenCode { path, model: None };
        let (out, _) = run(
            &provider,
            dir.path(),
            Duration::from_secs(20),
            Some(Duration::from_millis(200)),
        )
        .await;
        out.expect("a quiet opencode run completes");
    }

    #[test]
    fn idle_limit_defaults_to_ten_minutes_and_zero_turns_it_off() {
        assert_eq!(agent_idle_timeout(None), Some(Duration::from_secs(600)));
        assert_eq!(
            agent_idle_timeout(Some("nonsense")),
            Some(Duration::from_secs(600))
        );
        assert_eq!(
            agent_idle_timeout(Some(" 120 ")),
            Some(Duration::from_secs(120))
        );
        assert_eq!(agent_idle_timeout(Some("0")), None);
    }
}
