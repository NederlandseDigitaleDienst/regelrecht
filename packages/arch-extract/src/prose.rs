//! The **prose sidecar**: per-node "wat/waarom" narrative that lives *beside*
//! the code-derived model rather than inside the generator.
//!
//! The structure of the architecture is derived automatically (crate → module →
//! type → …); the *narrative* — what a part is for and why it exists — is
//! written by hand (or by an agent) and would only rot if we tried to generate
//! it. So it is kept in a sidecar, one Markdown file per node, keyed by the
//! model's stable **node id** (`crate:engine`, `mod:engine::service`, …). The
//! explorer overlays this prose on a node; a scheduled drift check (see
//! [`Drift`]) diffs the on-demand model against the sidecar and opens a PR with
//! proposals when the two drift apart.
//!
//! ## File format
//!
//! Each entry is a Markdown file under `packages/arch-extract/prose/`:
//!
//! ```text
//! ---
//! node: crate:engine
//! fingerprint: 3f9a1c0b7e5d2a84
//! ---
//! De law-execution engine … (vrije Markdown-tekst: wat en waarom)
//! ```
//!
//! The `node:` field is the authoritative key (the filename is only a readable
//! slug). `fingerprint` captures the shape of the node the prose was written
//! against; when the node's shape changes, the fingerprint no longer matches and
//! the entry is flagged **stale** so its prose can be revisited.
//!
//! ## Scope
//!
//! The full model has ~2200 nodes; per-node prose for every method and function
//! is neither realistic nor useful. The sidecar deliberately covers only the
//! **container** and **component** levels (crates/binaries and
//! modules/types) — [`in_scope`] — and skips the `code` level.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::model::{Kind, Level, Model, Node};

/// Repo-relative directory holding the prose sidecar files.
pub const PROSE_DIR: &str = "packages/arch-extract/prose";

/// Whether a node is within the sidecar's scope. Prose is maintained at the
/// container and component levels only; `code`-level nodes (methods, free
/// functions) are intentionally excluded — there are far too many and their
/// intent is captured well enough by their doc-comments in the model itself.
pub fn in_scope(node: &Node) -> bool {
    matches!(node.level, Level::Container | Level::Component)
}

/// A parsed sidecar entry: the shape fingerprint the prose was written against
/// plus the narrative body, and the file it came from.
#[derive(Debug, Clone)]
pub struct ProseEntry {
    pub node: String,
    pub fingerprint: String,
    pub body: String,
    pub path: PathBuf,
}

impl ProseEntry {
    /// A body that carries actual narrative (not just whitespace or an
    /// HTML-comment placeholder left by `sync`). Only entries with real prose
    /// are shown in the explorer and count as "documented".
    pub fn has_prose(&self) -> bool {
        !stripped_body_is_empty(&self.body)
    }
}

/// Treats a body as empty when, after removing HTML comments and whitespace,
/// nothing remains — so a scaffolded stub (`<!-- TODO … -->`) reads as "no
/// prose yet".
fn stripped_body_is_empty(body: &str) -> bool {
    let mut rest = body;
    let mut out = String::new();
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().is_empty()
}

/// A stable, platform-independent fingerprint of the parts of a node whose
/// change should prompt a prose review: its kind, level, name, source path and
/// doc-comment. Uses FNV-1a (64-bit) so the value is reproducible across
/// machines and Rust versions — a committed fingerprint must mean the same
/// thing everywhere, unlike the standard library's `DefaultHasher`.
pub fn fingerprint(node: &Node) -> String {
    let doc = node.doc.as_deref().unwrap_or("");
    let material = format!(
        "{}|{}|{}|{}|{}",
        kind_str(node.kind),
        level_str(node.level),
        node.name,
        node.path,
        doc
    );
    format!("{:016x}", fnv1a64(material.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn kind_str(kind: Kind) -> &'static str {
    match kind {
        Kind::Crate => "crate",
        Kind::Binary => "binary",
        Kind::Module => "module",
        Kind::Struct => "struct",
        Kind::Enum => "enum",
        Kind::Trait => "trait",
        Kind::Method => "method",
        Kind::Fn => "fn",
        Kind::App => "app",
        Kind::Dir => "dir",
        Kind::Component => "component",
        Kind::Composable => "composable",
    }
}

fn level_str(level: Level) -> &'static str {
    match level {
        Level::System => "system",
        Level::Container => "container",
        Level::Component => "component",
        Level::Code => "code",
    }
}

/// A readable, filesystem-safe filename for a node id. The node id itself stays
/// authoritative inside the file's frontmatter, so this only has to be unique
/// enough to avoid collisions between the ids we key on. Non-alphanumeric runs
/// collapse to a single `-`.
pub fn slug(node_id: &str) -> String {
    let mut out = String::with_capacity(node_id.len());
    let mut last_dash = false;
    for ch in node_id.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Renders a sidecar file's text (frontmatter + body).
fn render_file(node_id: &str, fingerprint: &str, body: &str) -> String {
    let body = body.trim_end();
    let mut s = String::new();
    s.push_str("---\n");
    let _ = writeln!(s, "node: {node_id}");
    let _ = writeln!(s, "fingerprint: {fingerprint}");
    s.push_str("---\n");
    if body.is_empty() {
        // A freshly scaffolded stub: an explicit TODO so the file reads as
        // "prose still to be written" both to a human and to `has_prose`.
        s.push_str("<!-- TODO: beschrijf wat dit onderdeel doet (wat) en waarom het bestaat (waarom). -->\n");
    } else {
        s.push_str(body);
        s.push('\n');
    }
    s
}

/// The body a scaffolded stub gets: a TODO plus, when the node already carries a
/// doc-comment in the model, that line as a concrete starting point. Everything
/// is inside HTML comments, so the stub still reads as "no prose yet"
/// ([`ProseEntry::has_prose`] is false) — the hint is for the author, not the
/// explorer.
fn stub_body(node: &Node) -> String {
    let mut s = String::from(
        "<!-- TODO: beschrijf wat dit onderdeel doet (wat) en waarom het bestaat (waarom). -->",
    );
    if let Some(doc) = node.doc.as_deref().filter(|d| !d.trim().is_empty()) {
        let _ = write!(s, "\n<!-- Doc-comment als startpunt: {doc} -->");
    }
    s
}

/// Parses one sidecar file. Returns `None` (with a warning) when the frontmatter
/// is missing or has no `node:` key — a malformed file is skipped rather than
/// aborting the whole load.
fn parse_file(path: &Path, text: &str) -> Option<ProseEntry> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    let front = &rest[..end];
    // Body starts after the closing `---` line.
    let after = &rest[end..];
    let body = after
        .strip_prefix("\n---\n")
        .or_else(|| after.strip_prefix("\n---\r\n"))
        .or_else(|| after.strip_prefix("\n---"))
        .unwrap_or("")
        .trim_start_matches(['\n', '\r'])
        .trim_end()
        .to_string();

    let mut node = None;
    let mut fingerprint = String::new();
    for line in front.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("node:") {
            node = Some(v.trim().trim_matches(['"', '\'']).to_string());
        } else if let Some(v) = line.strip_prefix("fingerprint:") {
            fingerprint = v.trim().trim_matches(['"', '\'']).to_string();
        }
    }

    let node = node?;
    Some(ProseEntry {
        node,
        fingerprint,
        body,
        path: path.to_path_buf(),
    })
}

/// Loads every `*.md` file from a prose directory, keyed by node id. A missing
/// directory yields an empty map (no prose written yet). Duplicate node ids are
/// an error — two files must never claim the same node.
pub fn load_prose(dir: &Path) -> Result<BTreeMap<String, ProseEntry>, String> {
    let mut map: BTreeMap<String, ProseEntry> = BTreeMap::new();
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(map),
        Err(e) => return Err(format!("reading {}: {e}", dir.display())),
    };
    for entry in read {
        let entry = entry.map_err(|e| format!("reading {}: {e}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let Some(parsed) = parse_file(&path, &text) else {
            eprintln!(
                "arch-extract prose: skipping {} (no `node:` frontmatter)",
                path.display()
            );
            continue;
        };
        if let Some(prev) = map.insert(parsed.node.clone(), parsed) {
            return Err(format!(
                "duplicate prose for node `{}` in {} and {}",
                prev.node,
                prev.path.display(),
                dir.display()
            ));
        }
    }
    Ok(map)
}

/// The result of diffing the model against the prose sidecar. All lists are
/// sorted by node id for a deterministic report and PR body.
#[derive(Debug, Default)]
pub struct Drift {
    /// In-scope model nodes with no prose (no file, or only a stub).
    pub missing: Vec<Node>,
    /// Prose entries whose node no longer exists in the model (removed/renamed).
    pub orphaned: Vec<ProseEntry>,
    /// Prose entries whose node still exists but changed shape since the prose
    /// was written (fingerprint mismatch) — the text may be out of date.
    pub stale: Vec<StaleEntry>,
}

/// A stale entry, pairing the recorded fingerprint with the node's current one.
#[derive(Debug, Clone)]
pub struct StaleEntry {
    pub node: String,
    pub was: String,
    pub now: String,
    pub path: PathBuf,
}

impl Drift {
    /// True when the sidecar is fully in sync with the model.
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.orphaned.is_empty() && self.stale.is_empty()
    }

    pub fn total(&self) -> usize {
        self.missing.len() + self.orphaned.len() + self.stale.len()
    }
}

/// Diffs a generated model against a loaded prose sidecar.
pub fn compute_drift(model: &Model, prose: &BTreeMap<String, ProseEntry>) -> Drift {
    let node_ids: BTreeMap<&str, &Node> = model.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut drift = Drift::default();

    // Missing / stale: walk the in-scope model nodes.
    for node in &model.nodes {
        if !in_scope(node) {
            continue;
        }
        match prose.get(&node.id) {
            None => drift.missing.push(node.clone()),
            Some(entry) if !entry.has_prose() => drift.missing.push(node.clone()),
            Some(entry) => {
                let now = fingerprint(node);
                if entry.fingerprint != now {
                    drift.stale.push(StaleEntry {
                        node: node.id.clone(),
                        was: entry.fingerprint.clone(),
                        now,
                        path: entry.path.clone(),
                    });
                }
            }
        }
    }

    // Orphaned: prose whose node is gone from the model entirely.
    for entry in prose.values() {
        if !node_ids.contains_key(entry.node.as_str()) {
            drift.orphaned.push(entry.clone());
        }
    }

    drift.missing.sort_by(|a, b| a.id.cmp(&b.id));
    drift.orphaned.sort_by(|a, b| a.node.cmp(&b.node));
    drift.stale.sort_by(|a, b| a.node.cmp(&b.node));
    drift
}

/// Writes a stub file (frontmatter with the current fingerprint, empty body)
/// for every in-scope model node that has **no** file yet. Returns the created
/// node ids. Existing files — including hand-written prose and earlier stubs —
/// are left untouched, so `sync` is idempotent and never clobbers narrative.
pub fn sync_stubs(
    model: &Model,
    prose: &BTreeMap<String, ProseEntry>,
    dir: &Path,
) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let mut created = Vec::new();
    for node in &model.nodes {
        if !in_scope(node) || prose.contains_key(&node.id) {
            continue;
        }
        let file = dir.join(format!("{}.md", slug(&node.id)));
        if file.exists() {
            continue;
        }
        let text = render_file(&node.id, &fingerprint(node), &stub_body(node));
        std::fs::write(&file, text).map_err(|e| format!("writing {}: {e}", file.display()))?;
        created.push(node.id.clone());
    }
    created.sort();
    Ok(created)
}

/// Rewrites the fingerprint of existing entries to match the current model,
/// preserving the body. Used after prose has been (re)written to clear a stale
/// flag. `ids` limits the operation; an empty slice blesses every entry that
/// still maps to a model node. Returns the blessed node ids.
pub fn bless(
    model: &Model,
    prose: &BTreeMap<String, ProseEntry>,
    ids: &[String],
) -> Result<Vec<String>, String> {
    let node_by_id: BTreeMap<&str, &Node> =
        model.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let selected: Option<std::collections::BTreeSet<&str>> = if ids.is_empty() {
        None
    } else {
        Some(ids.iter().map(String::as_str).collect())
    };

    let mut blessed = Vec::new();
    for entry in prose.values() {
        if let Some(sel) = &selected
            && !sel.contains(entry.node.as_str())
        {
            continue;
        }
        let Some(node) = node_by_id.get(entry.node.as_str()) else {
            continue;
        };
        let fp = fingerprint(node);
        if fp == entry.fingerprint {
            continue;
        }
        let text = render_file(&entry.node, &fp, &entry.body);
        std::fs::write(&entry.path, text)
            .map_err(|e| format!("writing {}: {e}", entry.path.display()))?;
        blessed.push(entry.node.clone());
    }
    blessed.sort();
    Ok(blessed)
}

// --- CLI (`arch-extract prose …`) ------------------------------------------

use std::process::ExitCode;

use crate::build::{self, DeepScope};

/// Parsed common options for the prose subcommands.
struct ProseArgs {
    manifest_path: Option<PathBuf>,
    prose_dir: Option<PathBuf>,
    json: bool,
    all: bool,
    ids: Vec<String>,
}

fn parse_prose_args(args: &[String]) -> Result<ProseArgs, String> {
    let mut manifest_path = None;
    let mut prose_dir = None;
    let mut json = false;
    let mut all = false;
    let mut ids = Vec::new();

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--all" => all = true,
            "--manifest-path" => {
                manifest_path = Some(PathBuf::from(
                    it.next().ok_or("--manifest-path needs a value")?,
                ));
            }
            "--prose-dir" => {
                prose_dir = Some(PathBuf::from(it.next().ok_or("--prose-dir needs a value")?));
            }
            other if other.starts_with("--") => {
                return Err(format!("unexpected argument: {other}"));
            }
            other => ids.push(other.to_string()),
        }
    }

    Ok(ProseArgs {
        manifest_path,
        prose_dir,
        json,
        all,
        ids,
    })
}

/// Entry point for `arch-extract prose <subcommand>`.
pub fn run(args: &[String]) -> ExitCode {
    let (sub, rest) = match args.split_first() {
        Some((s, r)) => (s.as_str(), r),
        None => {
            eprintln!("{PROSE_USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if matches!(sub, "-h" | "--help") {
        println!("{PROSE_USAGE}");
        return ExitCode::SUCCESS;
    }

    let parsed = match parse_prose_args(rest) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("arch-extract prose: {e}");
            return ExitCode::FAILURE;
        }
    };

    match dispatch(sub, &parsed) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("arch-extract prose: error: {e}");
            ExitCode::FAILURE
        }
    }
}

const PROSE_USAGE: &str = "\
arch-extract prose <status|check|sync|bless> [options]

  status            Report drift between the generated model and the prose sidecar.
  check             Like status, but exit non-zero when there is any drift (for the
                    scheduled flow to decide whether to open a PR).
  sync              Scaffold empty stub files for in-scope nodes without prose.
  bless [<id>…]     Refresh the stored fingerprint of existing entries (clears a
                    stale flag after the prose was rewritten). --all blesses every
                    entry.

Options:
  --json                 Machine-readable output (status/check).
  --manifest-path <p>    Workspace manifest for `cargo metadata` (default: discovered).
  --prose-dir <d>        Prose sidecar directory (default: <repo>/packages/arch-extract/prose).
  --all                  bless: refresh every entry.";

fn dispatch(sub: &str, args: &ProseArgs) -> Result<ExitCode, String> {
    let (model, repo_root) = build::build_model(args.manifest_path.as_deref(), &DeepScope::All)
        .map_err(|e| e.to_string())?;
    let dir = args
        .prose_dir
        .clone()
        .unwrap_or_else(|| repo_root.join(PROSE_DIR));
    let prose = load_prose(&dir)?;

    match sub {
        "status" | "check" => {
            let drift = compute_drift(&model, &prose);
            if args.json {
                print!("{}", drift_json(&drift));
            } else {
                print!("{}", drift_report(&drift, &model, &prose));
            }
            if sub == "check" && !drift.is_clean() {
                return Ok(ExitCode::FAILURE);
            }
            Ok(ExitCode::SUCCESS)
        }
        "sync" => {
            let created = sync_stubs(&model, &prose, &dir)?;
            eprintln!(
                "arch-extract prose: wrote {} stub(s) to {}",
                created.len(),
                dir.display()
            );
            for id in &created {
                println!("{id}");
            }
            Ok(ExitCode::SUCCESS)
        }
        "bless" => {
            let ids = if args.all {
                Vec::new()
            } else {
                args.ids.clone()
            };
            if !args.all && ids.is_empty() {
                return Err("bless needs node ids or --all".to_string());
            }
            let blessed = bless(&model, &prose, &ids)?;
            eprintln!("arch-extract prose: blessed {} entr(ies)", blessed.len());
            for id in &blessed {
                println!("{id}");
            }
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown prose subcommand: {other}")),
    }
}

/// Counts of in-scope nodes that are documented vs. total, for the report.
fn coverage(model: &Model, prose: &BTreeMap<String, ProseEntry>) -> (usize, usize) {
    let mut documented = 0;
    let mut total = 0;
    for node in &model.nodes {
        if !in_scope(node) {
            continue;
        }
        total += 1;
        if prose.get(&node.id).is_some_and(ProseEntry::has_prose) {
            documented += 1;
        }
    }
    (documented, total)
}

fn drift_report(drift: &Drift, model: &Model, prose: &BTreeMap<String, ProseEntry>) -> String {
    let (documented, total) = coverage(model, prose);
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Prose coverage: {documented}/{total} in-scope nodes documented (container + component)."
    );
    let _ = writeln!(
        s,
        "Drift: {} total ({} missing, {} stale, {} orphaned).",
        drift.total(),
        drift.missing.len(),
        drift.stale.len(),
        drift.orphaned.len()
    );
    if drift.is_clean() {
        let _ = writeln!(s, "\nSidecar is in sync with the model.");
        return s;
    }
    if !drift.missing.is_empty() {
        let _ = writeln!(s, "\nMissing prose (new / undocumented nodes):");
        for n in &drift.missing {
            let _ = writeln!(s, "  + {}  ({})", n.id, n.path);
        }
    }
    if !drift.stale.is_empty() {
        let _ = writeln!(
            s,
            "\nStale prose (node changed since the text was written):"
        );
        for e in &drift.stale {
            let _ = writeln!(
                s,
                "  ~ {}  ({} → {})  {}",
                e.node,
                e.was,
                e.now,
                e.path.display()
            );
        }
    }
    if !drift.orphaned.is_empty() {
        let _ = writeln!(s, "\nOrphaned prose (node no longer in the model):");
        for e in &drift.orphaned {
            let _ = writeln!(s, "  - {}  ({})", e.node, e.path.display());
        }
    }
    s
}

/// A small hand-rolled JSON report (no serde derive needed) for the scheduled
/// flow to consume.
fn drift_json(drift: &Drift) -> String {
    fn arr(items: impl Iterator<Item = String>) -> String {
        let body: Vec<String> = items.map(|s| format!("{s:?}")).collect();
        format!("[{}]", body.join(","))
    }
    format!(
        "{{\"clean\":{},\"missing\":{},\"stale\":{},\"orphaned\":{}}}\n",
        drift.is_clean(),
        arr(drift.missing.iter().map(|n| n.id.clone())),
        arr(drift.stale.iter().map(|e| e.node.clone())),
        arr(drift.orphaned.iter().map(|e| e.node.clone())),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn node(id: &str, level: Level, kind: Kind, doc: Option<&str>) -> Node {
        Node {
            id: id.to_string(),
            level,
            kind,
            lang: "rust".to_string(),
            name: id.rsplit(':').next().unwrap_or(id).to_string(),
            path: format!("packages/{}/src/lib.rs", id),
            parent: None,
            doc: doc.map(str::to_string),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        let a = node("crate:engine", Level::Container, Kind::Crate, Some("doc"));
        let same = node("crate:engine", Level::Container, Kind::Crate, Some("doc"));
        assert_eq!(fingerprint(&a), fingerprint(&same));

        let doc_changed = node("crate:engine", Level::Container, Kind::Crate, Some("new"));
        assert_ne!(fingerprint(&a), fingerprint(&doc_changed));
    }

    #[test]
    fn slug_is_filesystem_safe_and_readable() {
        assert_eq!(slug("crate:engine"), "crate-engine");
        assert_eq!(slug("mod:engine::service"), "mod-engine-service");
        assert_eq!(
            slug("type:engine::service::LawExecutionService"),
            "type-engine-service-lawexecutionservice"
        );
    }

    #[test]
    fn parse_roundtrips_frontmatter_and_body() {
        let text = render_file("crate:engine", "deadbeefdeadbeef", "De engine.\n\nWaarom.");
        let parsed = parse_file(Path::new("x.md"), &text).expect("parse");
        assert_eq!(parsed.node, "crate:engine");
        assert_eq!(parsed.fingerprint, "deadbeefdeadbeef");
        assert_eq!(parsed.body, "De engine.\n\nWaarom.");
        assert!(parsed.has_prose());
    }

    #[test]
    fn stub_body_reads_as_no_prose() {
        let text = render_file("crate:engine", "abc", "");
        let parsed = parse_file(Path::new("x.md"), &text).expect("parse");
        assert!(!parsed.has_prose(), "a TODO stub is not real prose");
    }

    #[test]
    fn drift_detects_missing_orphaned_and_stale() {
        let n_engine = node("crate:engine", Level::Container, Kind::Crate, Some("d1"));
        let n_mod = node(
            "mod:engine::svc",
            Level::Component,
            Kind::Module,
            Some("d2"),
        );
        // A code-level node must be out of scope and never counted as missing.
        let n_fn = node("fn:engine::svc::run", Level::Code, Kind::Fn, None);
        let model = Model::new(vec![n_engine.clone(), n_mod.clone(), n_fn], vec![]);

        let mut prose = BTreeMap::new();
        // engine: documented but with an outdated fingerprint -> stale.
        prose.insert(
            "crate:engine".to_string(),
            ProseEntry {
                node: "crate:engine".to_string(),
                fingerprint: "0000000000000000".to_string(),
                body: "De engine.".to_string(),
                path: PathBuf::from("crate-engine.md"),
            },
        );
        // a gone node -> orphaned.
        prose.insert(
            "crate:ghost".to_string(),
            ProseEntry {
                node: "crate:ghost".to_string(),
                fingerprint: "abc".to_string(),
                body: "Weg.".to_string(),
                path: PathBuf::from("crate-ghost.md"),
            },
        );
        // mod:engine::svc has no entry -> missing.

        let drift = compute_drift(&model, &prose);
        assert_eq!(
            drift
                .missing
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>(),
            vec!["mod:engine::svc"]
        );
        assert_eq!(
            drift
                .orphaned
                .iter()
                .map(|e| e.node.as_str())
                .collect::<Vec<_>>(),
            vec!["crate:ghost"]
        );
        assert_eq!(
            drift
                .stale
                .iter()
                .map(|e| e.node.as_str())
                .collect::<Vec<_>>(),
            vec!["crate:engine"]
        );
        assert!(!drift.is_clean());
        assert_eq!(drift.total(), 3);
    }

    #[test]
    fn sync_creates_stubs_then_bless_clears_stale() {
        let dir = std::env::temp_dir().join(format!("arch-prose-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let n = node("crate:engine", Level::Container, Kind::Crate, Some("d1"));
        let model = Model::new(vec![n.clone()], vec![]);

        // First sync: one stub created, which reads as still-missing prose.
        let empty = BTreeMap::new();
        let created = sync_stubs(&model, &empty, &dir).expect("sync");
        assert_eq!(created, vec!["crate:engine".to_string()]);
        let reloaded = load_prose(&dir).expect("load");
        assert_eq!(compute_drift(&model, &reloaded).missing.len(), 1);

        // Second sync is idempotent (file already exists).
        let created2 = sync_stubs(&model, &reloaded, &dir).expect("sync2");
        assert!(created2.is_empty());

        // Write real prose against an outdated fingerprint -> stale, then bless.
        let file = dir.join("crate-engine.md");
        std::fs::write(
            &file,
            render_file("crate:engine", "0000000000000000", "Echt."),
        )
        .unwrap();
        let reloaded = load_prose(&dir).expect("load");
        assert_eq!(compute_drift(&model, &reloaded).stale.len(), 1);
        let blessed = bless(&model, &reloaded, &[]).expect("bless");
        assert_eq!(blessed, vec!["crate:engine".to_string()]);
        let reloaded = load_prose(&dir).expect("load");
        assert!(compute_drift(&model, &reloaded).is_clean());
        assert_eq!(reloaded["crate:engine"].body, "Echt.");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
