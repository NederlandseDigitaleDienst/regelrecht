//! Integrity report over a traject's own corpus repo: the *content* half of
//! the in-band diagnosis.
//!
//! # Why this exists
//!
//! The corpus index enumerates a GitHub-backed source by **path**, without
//! reading a single body: a law's id is taken to be its directory name (see
//! `SourceMap::load_metadata_entry`). The editor, once a body is loaded,
//! re-resolves that law to the `$id` inside the YAML. When the two disagree
//! the id the editor asks for is an id the index has never heard of, and every
//! follow-up — enrich, the scenario list, the version list — comes back 404.
//! Nothing on that path can say why: each individual answer ("not found") is a
//! perfectly ordinary one.
//!
//! [`crate::traject_index_diagnosis`] answers "may I read this repo at all".
//! This module is its counterpart: the repo reads fine — is what is *in* it
//! internally consistent? Both run in-band, on the caller's own read token,
//! for the same reason: the repo may be private and user-token-only, so the
//! only credential that can see it is the one on the request. And both apply
//! the same precedence when there is no personal token: the source's
//! server-side one, which is what the traject's ordinary reads authenticate
//! with.
//!
//! # Shape
//!
//! [`scan_own_source`] does all the I/O and reduces every file to a small,
//! content-derived [`LawFacts`] / [`ScenarioFacts`]. [`run_checks`] is a pure
//! function over that [`CorpusScan`] — no GitHub, no state — so every check has
//! a unit test rather than an integration fixture (same split as
//! `validation::validate_scopes` in the corpus crate).
//!
//! # Cost
//!
//! One Trees call enumerates the whole branch with a blob sha per file. Facts
//! are memoised on that sha in [`IntegrityCache`], so a second call on an
//! unchanged branch reads **no** bodies at all — it costs exactly the one
//! Trees request. After a push only the changed blobs are fetched.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Serialize;
use tokio::sync::RwLock;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_corpus::models::SourceType;
use regelrecht_corpus::source_map::{LawReferenceKind, collect_law_references};
use regelrecht_github::GithubClient;
use regelrecht_law_model::parse_law_header;

use crate::accounts::AccountRecord;
use crate::corpus_handlers::{
    extract_loaded_law_ids, extract_target_law_ids, require_traject_corpus_from_ref,
    resolve_own_read_token,
};
use crate::state::AppState;
use crate::traject_corpus::TrajectCorpus;

/// How many law/scenario bodies are fetched at once on a cold scan.
///
/// Sequential reads would make a first scan of a corpus-sized repo take a
/// minute; unbounded parallelism would fire a hundred Contents calls at a
/// shared, rate-limited token in one burst. Eight keeps a cold scan in the
/// low seconds while staying well inside GitHub's concurrency guidance.
const READ_CONCURRENCY: usize = 8;

/// Reserved subtree that holds note sidecars, not laws. RFC-018 puts it at
/// the repository root, which is the source root only for a source without
/// a subpath; a source with a subpath can still carry the legacy copy
/// under it (see `regelrecht_corpus::annotation_sidecar`). Either way its
/// `annotations/{law_id}/annotations.yaml` shape collides with the law
/// convention `{layer}/{law_id}/{date}.yaml`, so the index skips it (see
/// `github::group_best_versions`) and so must this scan — otherwise every
/// annotated law would be reported as a law without an `$id`.
const ANNOTATIONS_DIR: &str = "annotations";

// ---------------------------------------------------------------------------
// Report shape (the JSON the page renders)
// ---------------------------------------------------------------------------

/// How bad a finding is. Errors break something today; warnings point at
/// something that is probably a mistake but still works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// The closed set of problems this report can find.
///
/// A stable key, not a sentence: the frontend groups on it and an operator can
/// count occurrences across trajects by it, so the wording of the Dutch
/// message may change without breaking either. Adding a variant is a
/// deliberate act — there is no catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// A law's directory name differs from the `$id` in its YAML. The
    /// original cause this page was built for.
    DirectoryNameMismatch,
    /// A version file's date stem differs from its `valid_from`.
    FileNameMismatch,
    /// The layer directory differs from the body's `regulatory_layer`.
    LayerDirectoryMismatch,
    /// Two directories declare the same `$id`.
    DuplicateLawId,
    /// A `source.regulation` / `implements` target that exists nowhere in
    /// the federated traject corpus.
    UnresolvedLawReference,
    /// A scenario step naming a law id that exists nowhere in the federated
    /// traject corpus.
    UnresolvedScenarioReference,
    /// A `scenarios/` folder in which no feature file evaluates the law the
    /// folder sits next to.
    ScenarioDirectoryWithoutTarget,
    /// A file that could not be read, so the checks over it did not run.
    FileUnreadable,
}

impl FindingKind {
    /// Severity is a property of the *kind*, never chosen per finding, so the
    /// two can't drift apart across the call sites that raise them.
    fn severity(self) -> Severity {
        match self {
            // A scenario folder that targets nothing still runs — it is a
            // strong smell (usually a leftover after a rename), not a break.
            Self::ScenarioDirectoryWithoutTarget => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// Stable sort rank, so a report's order doesn't depend on the order the
    /// checks happen to run in.
    fn rank(self) -> u8 {
        match self {
            Self::DirectoryNameMismatch => 0,
            Self::DuplicateLawId => 1,
            Self::LayerDirectoryMismatch => 2,
            Self::FileNameMismatch => 3,
            Self::UnresolvedLawReference => 4,
            Self::UnresolvedScenarioReference => 5,
            Self::FileUnreadable => 6,
            Self::ScenarioDirectoryWithoutTarget => 7,
        }
    }
}

/// One problem found, with everything the reader needs to act on it: where it
/// sits, what is wrong, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub kind: FindingKind,
    /// Path of the offending file or directory, relative to the source root
    /// (so it reads the same as the paths in the repo's own tree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The law this is about, as far as it can be established.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub law_id: Option<String>,
    /// What is wrong, in Dutch, in one sentence.
    pub message: String,
    /// What to do about it, in Dutch, concretely enough to act on without
    /// knowing the corpus conventions.
    pub remedy: String,
}

/// `GET /api/trajects/{traject_ref}/integrity` response body.
#[derive(Debug, Serialize)]
pub struct IntegrityReport {
    pub traject_ref: String,
    /// The source that was read — the traject's writable-own repo.
    pub source_id: String,
    /// How many law files were inspected.
    pub checked_laws: usize,
    /// How many `.feature` files were inspected.
    pub checked_scenarios: usize,
    /// Empty when nothing is wrong. Errors first, then warnings.
    pub findings: Vec<Finding>,
}

// ---------------------------------------------------------------------------
// What a scan reduces the repo to
// ---------------------------------------------------------------------------

/// The content-derived facts one law file contributes to the checks.
///
/// Everything here comes from the body; nothing from the path. That split is
/// the point — every check compares one against the other.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct LawFacts {
    /// The top-level `$id`.
    pub declared_id: Option<String>,
    /// The top-level `regulatory_layer` (schema enum, upper case).
    pub regulatory_layer: Option<String>,
    /// The top-level `valid_from`. May be a `#`-reference rather than a date.
    pub valid_from: Option<String>,
    /// Every other law this body points at.
    pub references: Vec<(LawReferenceKind, String)>,
}

impl LawFacts {
    /// Reduce a law body to the facts the checks need.
    fn from_yaml(body: &str) -> Self {
        let header = parse_law_header(body);
        Self {
            declared_id: header.id,
            regulatory_layer: header.regulatory_layer,
            valid_from: header.valid_from,
            references: collect_law_references(body)
                .into_iter()
                .map(|r| (r.kind, r.law_id))
                .collect(),
        }
    }
}

/// The content-derived facts one `.feature` file contributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ScenarioFacts {
    /// Law ids the file *evaluates* (`I evaluate "…" of "<id>"`).
    pub evaluated_ids: Vec<String>,
    /// Law ids the file *loads* (`law "<id>" is loaded`).
    pub loaded_ids: Vec<String>,
}

impl ScenarioFacts {
    fn from_feature(body: &str) -> Self {
        Self {
            evaluated_ids: extract_target_law_ids(body),
            loaded_ids: extract_loaded_law_ids(body),
        }
    }

    /// Every law id the file names, in either role.
    fn referenced_ids(&self) -> impl Iterator<Item = &String> {
        self.evaluated_ids.iter().chain(self.loaded_ids.iter())
    }
}

/// One file the scan looked at: its source-root-relative path plus either the
/// facts read from it, or why they could not be read.
///
/// A read failure is data, not an abort: one throttled blob must not take down
/// the whole report, so it becomes its own finding and every other check keeps
/// running (with the honest caveat that this file was not inspected).
#[derive(Debug, Clone)]
pub(crate) struct ScannedFile<T> {
    pub relative_path: String,
    pub facts: Result<Arc<T>, String>,
}

/// Everything [`run_checks`] works from.
#[derive(Debug, Default)]
pub(crate) struct CorpusScan {
    pub laws: Vec<ScannedFile<LawFacts>>,
    pub scenarios: Vec<ScannedFile<ScenarioFacts>>,
    /// Law ids the traject's federated index knows — including the seed
    /// sources, which are already fully loaded and need no reading here.
    pub indexed_law_ids: HashSet<String>,
    /// The source's in-repo subpath (`regulation/nl/`, with trailing slash),
    /// or empty when the source is the repo root.
    ///
    /// Paths are kept source-relative everywhere the checks reason about them
    /// — the first segment being the regulatory layer is a structural rule,
    /// and a configurable prefix in front of it would only be in the way. It
    /// is put back for the *reader*: a remedy that says "rename this folder"
    /// has to name the folder as it appears in the repository, or the reader
    /// goes looking for it one directory too deep.
    pub path_prefix: String,
}

impl CorpusScan {
    /// A source-relative path as it appears in the repository.
    fn at(&self, relative_path: &str) -> String {
        format!("{}{relative_path}", self.path_prefix)
    }
}

// ---------------------------------------------------------------------------
// Path structure
// ---------------------------------------------------------------------------

/// A law file's path decomposed into the parts the checks compare against the
/// body: `{layer}/[{org}/…]{law_dir}/{stem}.yaml`, relative to the source root.
///
/// The optional organisation segment is real: the corpus places
/// `gemeentelijke_verordening/<gemeente>/<wet>/<datum>.yaml`. Only the first
/// segment (the layer) and the last directory (which the index uses as the law
/// id) carry meaning; whatever sits between them is free organisation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LawPathParts<'a> {
    /// First path segment — the regulatory-layer directory.
    pub layer_dir: &'a str,
    /// Last directory — the one the index reads as the law's `$id`.
    pub law_dir: &'a str,
    /// The law's directory path, e.g. `wet/participatiewet`.
    pub dir: &'a str,
    /// Filename without the `.yaml` suffix — a date, by convention.
    pub stem: &'a str,
}

/// Decompose a source-root-relative law path, or `None` when it isn't shaped
/// like a law file at all (too shallow, wrong extension, or the reserved
/// annotations subtree).
pub(crate) fn split_law_path(relative_path: &str) -> Option<LawPathParts<'_>> {
    let stem = relative_path.strip_suffix(".yaml")?;
    let parts: Vec<&str> = relative_path.split('/').collect();
    if parts.len() < 3 || parts[0] == ANNOTATIONS_DIR {
        return None;
    }
    let dir_len = relative_path.len() - parts[parts.len() - 1].len() - 1;
    Some(LawPathParts {
        layer_dir: parts[0],
        law_dir: parts[parts.len() - 2],
        dir: &relative_path[..dir_len],
        stem: &stem[dir_len + 1..],
    })
}

/// A scenario file's path decomposed: `{law_dir_path}/scenarios/{file}.feature`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ScenarioPathParts<'a> {
    /// The `scenarios/` folder itself, e.g. `wet/participatiewet/scenarios`.
    pub scenarios_dir: &'a str,
    /// The law directory the folder sits in, e.g. `wet/participatiewet`.
    pub law_dir_path: &'a str,
}

/// Decompose a source-root-relative scenario path, or `None` when it isn't a
/// `.feature` file directly inside a `scenarios/` folder that sits inside a
/// law directory.
pub(crate) fn split_scenario_path(relative_path: &str) -> Option<ScenarioPathParts<'_>> {
    let parts: Vec<&str> = relative_path.split('/').collect();
    if !relative_path.ends_with(".feature") || parts.len() < 3 {
        return None;
    }
    if parts[parts.len() - 2] != "scenarios" {
        return None;
    }
    let scenarios_len = relative_path.len() - parts[parts.len() - 1].len() - 1;
    let scenarios_dir = &relative_path[..scenarios_len];
    let law_dir_path = &scenarios_dir[..scenarios_len - "scenarios".len() - 1];
    Some(ScenarioPathParts {
        scenarios_dir,
        law_dir_path,
    })
}

// ---------------------------------------------------------------------------
// The checks (pure)
// ---------------------------------------------------------------------------

/// Run every check over a scan. Pure: no I/O, no clock, no state — the whole
/// point of reducing the repo to a [`CorpusScan`] first.
///
/// Findings come back deterministically ordered: errors before warnings, then
/// by kind, then by path.
pub(crate) fn run_checks(scan: &CorpusScan) -> Vec<Finding> {
    // Which `$id`s each law directory declares. Normally one per directory;
    // more than one means its version files disagree, which the mismatch
    // check below reports per (directory, id) pair.
    let mut declared_by_dir: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for law in &scan.laws {
        let (Some(parts), Ok(facts)) = (split_law_path(&law.relative_path), &law.facts) else {
            continue;
        };
        if let Some(id) = facts.declared_id.as_deref() {
            declared_by_dir.entry(parts.dir).or_default().insert(id);
        }
    }

    // The id universe references resolve against: what the index enumerated
    // PLUS what the bodies actually declare.
    //
    // The union matters. A law whose directory name differs from its `$id` is
    // in the index under the directory name only, so resolving references
    // against the index alone would report every pointer to that law as
    // dangling — a cascade of consequences on top of the one finding that
    // names the cause. The reference checks answer "does this law exist in
    // this corpus at all", and by its own `$id` it plainly does.
    let mut known_ids: HashSet<&str> = scan.indexed_law_ids.iter().map(String::as_str).collect();
    for ids in declared_by_dir.values() {
        known_ids.extend(ids.iter().copied());
    }

    let mut findings = Vec::new();
    check_unreadable_files(scan, &mut findings);
    check_directory_names(&declared_by_dir, &scan.path_prefix, &mut findings);
    check_duplicate_ids(&declared_by_dir, &scan.path_prefix, &mut findings);
    check_file_names(scan, &mut findings);
    check_layer_directories(scan, &mut findings);
    check_law_references(scan, &known_ids, &mut findings);
    check_scenario_references(scan, &known_ids, &mut findings);
    check_scenario_targets(scan, &declared_by_dir, &mut findings);

    findings.sort_by(|a, b| {
        (a.severity, a.kind.rank(), &a.path, &a.law_id, &a.message).cmp(&(
            b.severity,
            b.kind.rank(),
            &b.path,
            &b.law_id,
            &b.message,
        ))
    });
    findings
}

/// A file that could not be read leaves a hole in the report; say so rather
/// than let the reader believe the checks covered it.
fn check_unreadable_files(scan: &CorpusScan, out: &mut Vec<Finding>) {
    let unreadable = scan
        .laws
        .iter()
        .filter_map(|f| f.facts.as_ref().err().map(|e| (&f.relative_path, e)))
        .chain(
            scan.scenarios
                .iter()
                .filter_map(|f| f.facts.as_ref().err().map(|e| (&f.relative_path, e))),
        );
    for (relative_path, error) in unreadable {
        let path = scan.at(relative_path);
        out.push(Finding {
            severity: FindingKind::FileUnreadable.severity(),
            kind: FindingKind::FileUnreadable,
            path: Some(path.clone()),
            law_id: None,
            message: format!(
                "Het bestand '{path}' kon niet gelezen worden ({error}); de controles op dit \
                 bestand zijn daardoor niet uitgevoerd."
            ),
            remedy: "Laad de pagina opnieuw. Blijft het misgaan, controleer dan of het bestand \
                     nog op de traject-branch staat en of je toegang tot de repo hebt."
                .to_string(),
        });
    }
}

/// Check 1 — the directory name must equal the `$id`.
///
/// This is the one that started it: the index keys GitHub-backed laws on the
/// directory name without reading a byte, so a disagreement makes the law
/// unreachable under the id the editor asks for.
fn check_directory_names(
    declared_by_dir: &BTreeMap<&str, BTreeSet<&str>>,
    path_prefix: &str,
    out: &mut Vec<Finding>,
) {
    for (relative_dir, ids) in declared_by_dir {
        let law_dir = relative_dir.rsplit('/').next().unwrap_or(relative_dir);
        let dir = format!("{path_prefix}{relative_dir}");
        for id in ids {
            if *id == law_dir {
                continue;
            }
            let parent = dir.strip_suffix(law_dir).unwrap_or("");
            out.push(Finding {
                severity: FindingKind::DirectoryNameMismatch.severity(),
                kind: FindingKind::DirectoryNameMismatch,
                path: Some(dir.clone()),
                law_id: Some((*id).to_string()),
                message: format!(
                    "Het regelwerk in de map '{dir}' heeft '$id: {id}', maar de map heet '{law_dir}'. \
                     De index gebruikt de mapnaam als id, dus dit regelwerk is onvindbaar \
                     onder zijn eigen id: verrijken, versies en scenario's mislukken met \
                     'niet gevonden'."
                ),
                remedy: format!(
                    "Hernoem de map '{dir}' naar '{parent}{id}', zodat de mapnaam gelijk is aan \
                     het $id. Wil je juist de mapnaam aanhouden, verander dan het $id in de \
                     YAML naar '{law_dir}' — en pas overal mee waar dat regelwerk wordt aangeroepen."
                ),
            });
        }
    }
}

/// Check 4 — one `$id`, one directory.
fn check_duplicate_ids(
    declared_by_dir: &BTreeMap<&str, BTreeSet<&str>>,
    path_prefix: &str,
    out: &mut Vec<Finding>,
) {
    let mut dirs_by_id: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (dir, ids) in declared_by_dir {
        for id in ids {
            dirs_by_id
                .entry(id)
                .or_default()
                .push(format!("{path_prefix}{dir}"));
        }
    }
    for (id, dirs) in dirs_by_id {
        if dirs.len() < 2 {
            continue;
        }
        let list = dirs.join("', '");
        out.push(Finding {
            severity: FindingKind::DuplicateLawId.severity(),
            kind: FindingKind::DuplicateLawId,
            path: Some(dirs[0].clone()),
            law_id: Some(id.to_string()),
            message: format!(
                "Het id '{id}' wordt door meerdere mappen gedeclareerd: '{list}'. Bij een \
                 dubbel id houdt de index er één over; het andere regelwerk verdwijnt uit de \
                 bibliotheek."
            ),
            remedy: format!(
                "Geef elk regelwerk een eigen $id, of verwijder de map die er niet meer hoort te \
                 staan. Zijn het twee versies van hetzelfde regelwerk, zet ze dan als losse \
                 datumbestanden in één map '{id}'."
            ),
        });
    }
}

/// Check 2 — the file's date stem must equal `valid_from`.
///
/// A `#`-reference `valid_from` (resolved at run time from an action output)
/// has no literal date to compare against and is skipped; so is an absent one
/// (the schema does not require the field).
fn check_file_names(scan: &CorpusScan, out: &mut Vec<Finding>) {
    for law in &scan.laws {
        let (Some(parts), Ok(facts)) = (split_law_path(&law.relative_path), &law.facts) else {
            continue;
        };
        let Some(valid_from) = facts.valid_from.as_deref() else {
            continue;
        };
        if valid_from.starts_with('#') || valid_from == parts.stem {
            continue;
        }
        let path = scan.at(&law.relative_path);
        out.push(Finding {
            severity: FindingKind::FileNameMismatch.severity(),
            kind: FindingKind::FileNameMismatch,
            path: Some(path.clone()),
            law_id: facts.declared_id.clone(),
            message: format!(
                "Het bestand '{path}' heeft 'valid_from: {valid_from}', maar heet '{}.yaml'. De \
                 bestandsnaam bepaalt welke versie de index als geldig kiest, dus die keuze \
                 klopt hier niet met de versie zelf.",
                parts.stem
            ),
            remedy: format!(
                "Hernoem het bestand naar '{}/{valid_from}.yaml', of pas 'valid_from' in de YAML \
                 aan naar '{}'.",
                scan.at(parts.dir),
                parts.stem
            ),
        });
    }
}

/// Check 3 — the layer directory must equal `regulatory_layer`.
///
/// Compared case-insensitively: the schema enum is upper case
/// (`GEMEENTELIJKE_VERORDENING`), the directory convention lower case. Only
/// the first segment is the layer; an organisation segment between it and the
/// law directory is normal (`gemeentelijke_verordening/<gemeente>/<wet>/`).
fn check_layer_directories(scan: &CorpusScan, out: &mut Vec<Finding>) {
    // One finding per (directory, layer) pair, not per version file: five
    // versions in a misplaced folder are one move, not five.
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for law in &scan.laws {
        let (Some(parts), Ok(facts)) = (split_law_path(&law.relative_path), &law.facts) else {
            continue;
        };
        let Some(layer) = facts.regulatory_layer.as_deref() else {
            continue;
        };
        if layer.eq_ignore_ascii_case(parts.layer_dir) || !seen.insert((parts.dir, layer)) {
            continue;
        }
        let expected_dir = scan.at(&format!(
            "{}{}",
            layer.to_ascii_lowercase(),
            &parts.dir[parts.layer_dir.len()..]
        ));
        let dir = scan.at(parts.dir);
        out.push(Finding {
            severity: FindingKind::LayerDirectoryMismatch.severity(),
            kind: FindingKind::LayerDirectoryMismatch,
            path: Some(dir.clone()),
            law_id: facts.declared_id.clone(),
            message: format!(
                "Het regelwerk in '{dir}' heeft 'regulatory_layer: {layer}', maar staat in de laag-map \
                 '{}'. De mapindeling en het regelwerk spreken elkaar dus tegen.",
                parts.layer_dir
            ),
            remedy: format!(
                "Verplaats de map naar '{expected_dir}', of pas 'regulatory_layer' aan naar \
                 '{}'.",
                parts.layer_dir.to_ascii_uppercase()
            ),
        });
    }
}

/// Check 5 — every `source.regulation` / `implements` target must exist.
fn check_law_references(scan: &CorpusScan, known_ids: &HashSet<&str>, out: &mut Vec<Finding>) {
    for law in &scan.laws {
        let (Some(parts), Ok(facts)) = (split_law_path(&law.relative_path), &law.facts) else {
            continue;
        };
        let law_label = facts.declared_id.clone().unwrap_or_else(|| {
            // No `$id` to name it by: the directory is the next best handle,
            // and the reader can find the file from the path anyway.
            parts.law_dir.to_string()
        });
        for (kind, target) in &facts.references {
            if known_ids.contains(target.as_str()) {
                continue;
            }
            let via = match kind {
                LawReferenceKind::SourceRegulation => "verwijst via 'source.regulation' naar",
                LawReferenceKind::SourceArticle => "verwijst via 'source.article' naar",
                LawReferenceKind::Implements => "declareert 'implements' op",
            };
            out.push(Finding {
                severity: FindingKind::UnresolvedLawReference.severity(),
                kind: FindingKind::UnresolvedLawReference,
                path: Some(scan.at(&law.relative_path)),
                law_id: Some(law_label.clone()),
                message: format!(
                    "'{law_label}' {via} '{target}', maar geen enkel regelwerk in dit traject-corpus \
                     heeft dat $id. Een berekening die deze verwijzing volgt, loopt vast."
                ),
                remedy: format!(
                    "Corrigeer '{target}' naar het $id van het bedoelde regelwerk, of voeg dat regelwerk toe \
                     aan dit traject."
                ),
            });
        }
    }
}

/// Check 6 — every law id a scenario names must exist.
fn check_scenario_references(scan: &CorpusScan, known_ids: &HashSet<&str>, out: &mut Vec<Finding>) {
    for scenario in &scan.scenarios {
        let Ok(facts) = &scenario.facts else {
            continue;
        };
        for target in facts.referenced_ids() {
            if known_ids.contains(target.as_str()) {
                continue;
            }
            let path = scan.at(&scenario.relative_path);
            out.push(Finding {
                severity: FindingKind::UnresolvedScenarioReference.severity(),
                kind: FindingKind::UnresolvedScenarioReference,
                path: Some(path.clone()),
                law_id: Some(target.clone()),
                message: format!(
                    "Het scenario '{path}' noemt '{target}', maar geen enkel regelwerk in dit \
                     traject-corpus heeft dat $id. Dit scenario kan niet draaien."
                ),
                remedy: format!(
                    "Corrigeer '{target}' in de stap naar het $id van het bedoelde regelwerk, of voeg \
                     dat regelwerk toe aan dit traject."
                ),
            });
        }
    }
}

/// Check 7 — a `scenarios/` folder should test the law it sits next to.
///
/// Compared against the `$id` the neighbouring body *declares*, not the
/// directory name: when those two disagree, check 1 already owns that problem
/// and repeating it here as a second, differently-worded finding would send
/// the reader down the wrong path.
fn check_scenario_targets(
    scan: &CorpusScan,
    declared_by_dir: &BTreeMap<&str, BTreeSet<&str>>,
    out: &mut Vec<Finding>,
) {
    // Evaluated ids per `scenarios/` folder, plus the law directory it belongs
    // to. A folder with no readable feature file at all yields no group — the
    // unreadable-file finding covers that case.
    let mut evaluated_per_dir: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    for scenario in &scan.scenarios {
        let (Some(parts), Ok(facts)) = (
            split_scenario_path(&scenario.relative_path),
            &scenario.facts,
        ) else {
            continue;
        };
        let entry = evaluated_per_dir
            .entry((parts.scenarios_dir, parts.law_dir_path))
            .or_default();
        entry.extend(facts.evaluated_ids.iter().map(String::as_str));
    }

    for ((relative_scenarios_dir, law_dir_path), evaluated) in evaluated_per_dir {
        // Which law does this folder sit next to? Its declared `$id`s, or —
        // for a folder without a readable law file — the directory name.
        let neighbour_dir_name = law_dir_path.rsplit('/').next().unwrap_or(law_dir_path);
        let scenarios_dir = scan.at(relative_scenarios_dir);
        let expected: BTreeSet<&str> = declared_by_dir
            .get(law_dir_path)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_else(|| BTreeSet::from([neighbour_dir_name]));
        if expected.iter().any(|id| evaluated.contains(id)) {
            continue;
        }
        let target = expected.iter().copied().collect::<Vec<_>>().join("' of '");
        let evaluated_list = if evaluated.is_empty() {
            "geen enkel regelwerk".to_string()
        } else {
            format!(
                "alleen '{}'",
                evaluated.iter().copied().collect::<Vec<_>>().join("', '")
            )
        };
        out.push(Finding {
            severity: FindingKind::ScenarioDirectoryWithoutTarget.severity(),
            kind: FindingKind::ScenarioDirectoryWithoutTarget,
            path: Some(scenarios_dir.clone()),
            law_id: expected.iter().next().map(|id| (*id).to_string()),
            message: format!(
                "De scenario's in '{scenarios_dir}' evalueren {evaluated_list} — niet '{target}', \
                 het regelwerk waar deze map bij hoort. Dat regelwerk wordt hier dus niet getoetst."
            ),
            remedy: format!(
                "Voeg een stap toe die '{target}' evalueert (bijvoorbeeld: Then I evaluate \
                 \"<uitkomst>\" of \"{target}\"), of verplaats deze scenario's naar de map van \
                 het regelwerk dat ze wél evalueren."
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/// What one enumerated blob reduced to, keyed by its sha.
#[derive(Debug, Clone)]
enum CachedFacts {
    Law(Arc<LawFacts>),
    Scenario(Arc<ScenarioFacts>),
}

/// Content-addressed memo of the integrity scan, per traject.
///
/// The key is the blob sha the Trees enumeration reported: two enumerations
/// reporting the same sha are byte-identical, so a repeat scan without new
/// commits reuses every entry and reads no bodies at all. After a push only
/// the changed blobs miss.
///
/// Process-wide (on [`AppState`]) rather than on the per-traject index
/// snapshot on purpose: that snapshot is rebuilt every minute, which would
/// throw the memo away and re-read the whole repo on the next visit — exactly
/// what this must not do. Each scan swaps in a map holding only the shas it
/// just saw, so a traject's entry stays O(its repo) however much the content
/// churns; the outer map is bounded by the number of trajects visited.
#[derive(Default)]
pub struct IntegrityCache {
    per_traject: RwLock<HashMap<Uuid, Arc<HashMap<String, CachedFacts>>>>,
}

impl IntegrityCache {
    pub fn new() -> Self {
        Self::default()
    }

    async fn get(&self, traject_id: Uuid) -> Arc<HashMap<String, CachedFacts>> {
        self.per_traject
            .read()
            .await
            .get(&traject_id)
            .cloned()
            .unwrap_or_default()
    }

    async fn store(&self, traject_id: Uuid, memo: HashMap<String, CachedFacts>) {
        self.per_traject
            .write()
            .await
            .insert(traject_id, Arc::new(memo));
    }
}

// ---------------------------------------------------------------------------
// Scanning (the I/O half)
// ---------------------------------------------------------------------------

/// A file the enumeration found, before its body was read.
#[derive(Clone)]
struct EnumeratedFile {
    /// Path relative to the source root — what the report shows.
    relative_path: String,
    /// Path relative to the repo root — what GitHub is asked for.
    repo_path: String,
    /// Blob sha from the enumeration, when the source reports one. `None`
    /// means "no content identity available", so no memo entry either.
    sha: Option<String>,
    is_law: bool,
}

/// Enumerate the traject's own source and reduce every law/scenario file to
/// its facts, reusing everything the cache already knows by blob sha.
async fn scan_own_source(
    state: &AppState,
    traject: &TrajectCorpus,
    token: Option<&str>,
) -> Result<CorpusScan, (StatusCode, String)> {
    let source = traject.own_source().ok_or_else(|| {
        tracing::error!(
            traject = %traject.traject_id,
            source_id = %traject.writable_own_source_id,
            "traject has no writable-own source in its registry"
        );
        (
            StatusCode::BAD_GATEWAY,
            "De eigen repo van dit traject is niet geconfigureerd, dus er valt niets te \
             controleren."
                .to_string(),
        )
    })?;

    // The source's in-repo subpath, normalised to the `foo/bar/` form the
    // report prefixes paths with.
    let path_prefix = match &source.source_type {
        SourceType::GitHub { github } => github
            .path
            .as_deref()
            .map(|p| p.trim_matches('/'))
            .filter(|p| !p.is_empty())
            .map(|p| format!("{p}/"))
            .unwrap_or_default(),
        // A local source is rooted at its configured directory, so there is
        // nothing above the source root to name.
        SourceType::Local { .. } => String::new(),
    };

    let (files, bodies) = match &source.source_type {
        SourceType::GitHub { github } => {
            // From the traject's own settings, so the scan reaches the host
            // the traject's normal reads reach.
            let client = traject.corpus.github.client().map_err(|e| {
                tracing::error!(
                    traject = %traject.traject_id,
                    error = %e,
                    "integrity scan: failed to build GitHub client"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "De integriteitscontrole kon niet worden gestart.".to_string(),
                )
            })?;
            let client = Arc::new(client);
            let repo = github.full_repo();
            let git_ref = github.effective_ref().to_string();
            // One Trees call for the whole branch: paths plus a blob sha per
            // file, which is what makes the memo below possible at all.
            let entries = client
                .list_tree_files(&repo, &git_ref, token)
                .await
                .map_err(|e| github_read_error(traject.traject_id, &repo, e))?
                // `None` is the 304 "tree unchanged" answer, which needs a
                // previously cached ETag on this very client — and the client
                // is built one line above. Unreachable in practice; treat a
                // surprise as an empty enumeration rather than a panic.
                .unwrap_or_default();
            let files = classify_tree_entries(entries, github.path.as_deref());
            let bodies = BodyReader::Github {
                client,
                repo,
                git_ref,
                token: token.map(str::to_string),
            };
            (files, bodies)
        }
        // Local source (dev / preview stack): walk the checkout through the
        // backend. No blob shas, so no memo — reading a local tree is cheap
        // enough that re-reading it per request is not worth a second
        // caching scheme.
        SourceType::Local { .. } => {
            let Some(entry) = traject
                .corpus
                .backends
                .get(&traject.writable_own_source_id)
                .cloned()
            else {
                return Ok(CorpusScan {
                    indexed_law_ids: indexed_ids(traject),
                    path_prefix,
                    ..CorpusScan::default()
                });
            };
            let listing = {
                let backend = entry.backend.lock().await;
                backend
                    .list_files_recursive(Path::new(""), None)
                    .await
                    .map_err(|e| {
                        tracing::warn!(traject = %traject.traject_id, error = %e, "integrity scan: listing failed");
                        (
                            StatusCode::BAD_GATEWAY,
                            "De bestanden van dit traject konden niet worden opgesomd."
                                .to_string(),
                        )
                    })?
            };
            let files = classify_local_entries(listing);
            (files, BodyReader::Backend(entry.backend))
        }
    };

    let cached = state.integrity.get(traject.traject_id).await;
    let (scanned, fresh_memo) = read_facts(&files, &cached, &bodies).await;
    state.integrity.store(traject.traject_id, fresh_memo).await;

    let mut scan = CorpusScan {
        indexed_law_ids: indexed_ids(traject),
        path_prefix,
        ..CorpusScan::default()
    };
    for (file, facts) in files.iter().zip(scanned) {
        match facts {
            ScannedFacts::Law(facts) => scan.laws.push(ScannedFile {
                relative_path: file.relative_path.clone(),
                facts,
            }),
            ScannedFacts::Scenario(facts) => scan.scenarios.push(ScannedFile {
                relative_path: file.relative_path.clone(),
                facts,
            }),
        }
    }
    Ok(scan)
}

/// Law ids the traject's federated index knows about, across all its sources.
fn indexed_ids(traject: &TrajectCorpus) -> HashSet<String> {
    traject
        .corpus
        .source_map
        .laws()
        .map(|law| law.law_id.clone())
        .collect()
}

/// Narrow a Trees listing to the law and scenario files under the source's
/// in-repo subpath, in source-root-relative form.
fn classify_tree_entries(
    entries: Vec<regelrecht_github::TreeEntryFile>,
    subpath: Option<&str>,
) -> Vec<EnumeratedFile> {
    let prefix = subpath
        .map(|s| s.trim_end_matches('/'))
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}/"));
    let mut out = Vec::new();
    for entry in entries {
        let relative_path = match &prefix {
            Some(p) => match entry.path.strip_prefix(p.as_str()) {
                Some(rest) => rest.to_string(),
                None => continue,
            },
            None => entry.path.clone(),
        };
        let Some(is_law) = classify_relative_path(&relative_path) else {
            continue;
        };
        out.push(EnumeratedFile {
            relative_path,
            repo_path: entry.path,
            sha: entry.sha,
            is_law,
        });
    }
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    out
}

/// Same narrowing for a local checkout, whose listing is already
/// source-root-relative and carries no shas.
fn classify_local_entries(
    entries: Vec<regelrecht_corpus::backend::RecursiveFileEntry>,
) -> Vec<EnumeratedFile> {
    let mut out: Vec<EnumeratedFile> = entries
        .into_iter()
        .filter_map(|e| {
            let is_law = classify_relative_path(&e.relative_path)?;
            Some(EnumeratedFile {
                repo_path: e.relative_path.clone(),
                relative_path: e.relative_path,
                sha: None,
                is_law,
            })
        })
        .collect();
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    out
}

/// `Some(true)` for a law file, `Some(false)` for a scenario file, `None` for
/// everything else in the repo (documents, sidecars, READMEs, CI config).
fn classify_relative_path(relative_path: &str) -> Option<bool> {
    if split_law_path(relative_path).is_some() {
        Some(true)
    } else if split_scenario_path(relative_path).is_some() {
        Some(false)
    } else {
        None
    }
}

/// Where bodies come from for this scan. Cheap to clone (everything shared
/// behind an `Arc`) so a batch of reads can be spawned in parallel.
#[derive(Clone)]
enum BodyReader {
    Github {
        client: Arc<GithubClient>,
        repo: String,
        git_ref: String,
        token: Option<String>,
    },
    Backend(Arc<tokio::sync::Mutex<Box<dyn regelrecht_corpus::backend::RepoBackend>>>),
}

impl BodyReader {
    async fn read(&self, file: &EnumeratedFile) -> Result<String, String> {
        match self {
            Self::Github {
                client,
                repo,
                git_ref,
                token,
            } => client
                .fetch_file_raw(repo, git_ref, &file.repo_path, token.as_deref())
                .await
                .map_err(|e| short_error(&e.to_string())),
            // Local checkout: the backend mutex serialises these, which is
            // fine — a local read costs no round trip.
            Self::Backend(backend) => {
                let guard = backend.lock().await;
                match guard.read_file(Path::new(&file.relative_path)).await {
                    Ok(Some(body)) => Ok(body),
                    Ok(None) => Err("bestand niet gevonden".to_string()),
                    Err(e) => Err(short_error(&e.to_string())),
                }
            }
        }
    }
}

/// Per-file outcome, kept parallel to the enumeration so the caller can zip
/// them back together without re-deriving the law/scenario split.
enum ScannedFacts {
    Law(Result<Arc<LawFacts>, String>),
    Scenario(Result<Arc<ScenarioFacts>, String>),
}

impl ScannedFacts {
    /// Reduce a freshly-read body (or a read failure) to its facts.
    fn from_body(is_law: bool, body: Result<String, String>) -> Self {
        match (body, is_law) {
            (Ok(body), true) => Self::Law(Ok(Arc::new(LawFacts::from_yaml(&body)))),
            (Ok(body), false) => Self::Scenario(Ok(Arc::new(ScenarioFacts::from_feature(&body)))),
            (Err(e), true) => Self::Law(Err(e)),
            (Err(e), false) => Self::Scenario(Err(e)),
        }
    }

    /// The memo entry this outcome contributes, if any. A failed read is
    /// never memoised: the next call must retry rather than serve the
    /// failure for as long as the blob stays unchanged.
    fn to_cached(&self) -> Option<CachedFacts> {
        match self {
            Self::Law(Ok(facts)) => Some(CachedFacts::Law(facts.clone())),
            Self::Scenario(Ok(facts)) => Some(CachedFacts::Scenario(facts.clone())),
            _ => None,
        }
    }
}

/// Resolve every enumerated file to its facts: from the memo when the blob sha
/// is unchanged, otherwise by reading the body. Returns the per-file outcomes
/// (parallel to `files`) plus the memo for the next call, holding only the
/// shas seen now.
async fn read_facts(
    files: &[EnumeratedFile],
    cached: &HashMap<String, CachedFacts>,
    bodies: &BodyReader,
) -> (Vec<ScannedFacts>, HashMap<String, CachedFacts>) {
    let mut out: Vec<Option<ScannedFacts>> = (0..files.len()).map(|_| None).collect();
    let mut misses: Vec<usize> = Vec::new();

    for (i, file) in files.iter().enumerate() {
        // A hit of the wrong shape (a path that changed from law to scenario
        // under the same sha — an empty file, say) falls through to a re-read
        // rather than being forced into the wrong bucket.
        let hit = file.sha.as_ref().and_then(|sha| cached.get(sha));
        match (hit, file.is_law) {
            (Some(CachedFacts::Law(facts)), true) => {
                out[i] = Some(ScannedFacts::Law(Ok(facts.clone())));
            }
            (Some(CachedFacts::Scenario(facts)), false) => {
                out[i] = Some(ScannedFacts::Scenario(Ok(facts.clone())));
            }
            _ => misses.push(i),
        }
    }

    // Bodies are fetched in bounded-parallel batches: one at a time is too
    // slow on a cold, corpus-sized repo; all at once is a burst against a
    // shared rate limit. Each body is reduced to its (small) facts as soon as
    // it lands, so peak memory is one batch of bodies, never the whole corpus.
    for batch in misses.chunks(READ_CONCURRENCY) {
        let mut set = tokio::task::JoinSet::new();
        for &i in batch {
            let bodies = bodies.clone();
            let file = files[i].clone();
            set.spawn(async move {
                let body = bodies.read(&file).await;
                (i, ScannedFacts::from_body(file.is_law, body))
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((i, facts)) => out[i] = Some(facts),
                // A panicked read task is a bug, not a corpus problem; leave
                // the slot empty so it surfaces as an unreadable file below.
                Err(e) => tracing::error!(error = %e, "integrity scan: body read task failed"),
            }
        }
    }

    let mut memo: HashMap<String, CachedFacts> = HashMap::new();
    let out: Vec<ScannedFacts> = out
        .into_iter()
        .enumerate()
        .map(|(i, facts)| {
            facts.unwrap_or_else(|| {
                ScannedFacts::from_body(files[i].is_law, Err("lezen afgebroken".to_string()))
            })
        })
        .collect();
    for (file, facts) in files.iter().zip(&out) {
        if let (Some(sha), Some(entry)) = (&file.sha, facts.to_cached()) {
            memo.insert(sha.clone(), entry);
        }
    }
    (out, memo)
}

/// Trim a backend error to something that fits in a user-facing sentence.
/// The full text is logged by the layer that raised it; here it is context,
/// not the message.
fn short_error(error: &str) -> String {
    const MAX: usize = 120;
    let one_line = error.replace(['\n', '\r'], " ");
    if one_line.chars().count() <= MAX {
        return one_line;
    }
    one_line.chars().take(MAX - 1).chain(['…']).collect()
}

/// The Trees enumeration is the one call that can sink the whole report:
/// without it there is nothing to check. Answer 502 with a plain sentence and
/// log the raw cause for operators.
fn github_read_error(
    traject_id: Uuid,
    repo: &str,
    error: regelrecht_github::GithubError,
) -> (StatusCode, String) {
    tracing::warn!(
        traject = %traject_id,
        repo = %repo,
        error = %error,
        "integrity scan: could not enumerate the traject branch"
    );
    (
        StatusCode::BAD_GATEWAY,
        "De bestanden van dit traject konden niet bij GitHub worden opgehaald, dus de \
         integriteitscontrole kon niet draaien. Probeer het over een paar minuten opnieuw."
            .to_string(),
    )
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `GET /api/trajects/{traject_ref}/integrity`
///
/// Behind the same membership guard as every other traject route, and reading
/// with the same credential the traject's normal reads use — the caller's own
/// GitHub token when the source has no server-side one (a user-token repo is
/// never read with the central token).
pub async fn get_traject_integrity(
    State(state): State<AppState>,
    session: Session,
    Extension(account): Extension<AccountRecord>,
    UrlPath(traject_ref): UrlPath<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<IntegrityReport>, (StatusCode, String)> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let token = resolve_own_read_token(&state, account.id, &headers, &traject).await?;
    // No personal token means the backend reads this source with the
    // server-side credential configured for it — and `resolve_own_read_token`
    // answers `None` precisely because the backend carries that credential
    // itself. This scan bypasses the backend and talks to GitHub directly, so
    // it has to re-resolve that token or a private repo with a service token
    // would be enumerated anonymously and 404 every time. Same fallback, in
    // the same order, as the index diagnosis (`require_traject_index`).
    let token = match token {
        Some(tok) => Some(tok),
        None => traject.own_server_token(),
    };

    let scan = scan_own_source(&state, &traject, token.as_deref()).await?;
    let findings = run_checks(&scan);

    tracing::debug!(
        traject = %traject.traject_id,
        laws = scan.laws.len(),
        scenarios = scan.scenarios.len(),
        findings = findings.len(),
        "integrity report built"
    );

    Ok(Json(IntegrityReport {
        traject_ref,
        source_id: traject.writable_own_source_id.clone(),
        checked_laws: scan.laws.len(),
        checked_scenarios: scan.scenarios.len(),
        findings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fictional corpus fixtures throughout — no real repo, traject or law
    /// names (this is a public repository).
    fn law(path: &str, facts: LawFacts) -> ScannedFile<LawFacts> {
        ScannedFile {
            relative_path: path.to_string(),
            facts: Ok(Arc::new(facts)),
        }
    }

    fn scenario(path: &str, evaluated: &[&str], loaded: &[&str]) -> ScannedFile<ScenarioFacts> {
        ScannedFile {
            relative_path: path.to_string(),
            facts: Ok(Arc::new(ScenarioFacts {
                evaluated_ids: evaluated.iter().map(|s| s.to_string()).collect(),
                loaded_ids: loaded.iter().map(|s| s.to_string()).collect(),
            })),
        }
    }

    /// A well-formed law body's facts: everything agrees with the path
    /// `{layer}/{id}/{valid_from}.yaml`.
    fn healthy(id: &str, layer: &str, valid_from: &str) -> LawFacts {
        LawFacts {
            declared_id: Some(id.to_string()),
            regulatory_layer: Some(layer.to_string()),
            valid_from: Some(valid_from.to_string()),
            references: Vec::new(),
        }
    }

    fn scan(
        laws: Vec<ScannedFile<LawFacts>>,
        scenarios: Vec<ScannedFile<ScenarioFacts>>,
    ) -> CorpusScan {
        let indexed_law_ids = laws
            .iter()
            .filter_map(|l| split_law_path(&l.relative_path).map(|p| p.law_dir.to_string()))
            .collect();
        CorpusScan {
            laws,
            scenarios,
            indexed_law_ids,
            // Same subpath as the real corpus repo, so the fixtures prove
            // that findings name paths as they appear in the repository.
            path_prefix: "regulation/nl/".to_string(),
        }
    }

    fn kinds(findings: &[Finding]) -> Vec<FindingKind> {
        findings.iter().map(|f| f.kind).collect()
    }

    fn of_kind(findings: &[Finding], kind: FindingKind) -> Vec<&Finding> {
        findings.iter().filter(|f| f.kind == kind).collect()
    }

    // --- path splitting ---

    #[test]
    fn law_paths_split_with_and_without_an_organisation_segment() {
        let plain = split_law_path("wet/wet_alpha/2025-01-01.yaml").expect("law path");
        assert_eq!(plain.layer_dir, "wet");
        assert_eq!(plain.law_dir, "wet_alpha");
        assert_eq!(plain.dir, "wet/wet_alpha");
        assert_eq!(plain.stem, "2025-01-01");

        let nested = split_law_path("waterschaps_verordening/hoogland/keur_alpha/2025-01-01.yaml")
            .expect("law path");
        assert_eq!(nested.layer_dir, "waterschaps_verordening");
        assert_eq!(nested.law_dir, "keur_alpha");
        assert_eq!(nested.dir, "waterschaps_verordening/hoogland/keur_alpha");
    }

    #[test]
    fn non_law_paths_are_not_mistaken_for_laws() {
        // Too shallow, wrong extension, and the reserved annotations subtree.
        assert!(split_law_path("wet/2025-01-01.yaml").is_none());
        assert!(split_law_path("wet/wet_alpha/README.md").is_none());
        assert!(split_law_path("annotations/wet_alpha/annotations.yaml").is_none());
    }

    #[test]
    fn scenario_paths_split_into_folder_and_neighbouring_law_dir() {
        let parts =
            split_scenario_path("wet/wet_alpha/scenarios/basis.feature").expect("scenario path");
        assert_eq!(parts.scenarios_dir, "wet/wet_alpha/scenarios");
        assert_eq!(parts.law_dir_path, "wet/wet_alpha");
        // A feature file that isn't inside a `scenarios/` folder isn't one.
        assert!(split_scenario_path("wet/wet_alpha/basis.feature").is_none());
    }

    // --- check 1: directory name vs $id ---

    #[test]
    fn directory_name_matching_the_id_is_clean() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2025-01-01.yaml",
                healthy("wet_alpha", "WET", "2025-01-01"),
            )],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn directory_name_differing_from_the_id_is_an_error_with_a_rename_remedy() {
        let findings = run_checks(&scan(
            vec![law(
                "waterschaps_verordening/hoogland/keur_alpha/2025-01-01.yaml",
                healthy(
                    "keur_alpha_hoogland",
                    "WATERSCHAPS_VERORDENING",
                    "2025-01-01",
                ),
            )],
            vec![],
        ));
        assert_eq!(kinds(&findings), vec![FindingKind::DirectoryNameMismatch]);
        let f = &findings[0];
        assert_eq!(f.severity, Severity::Error);
        // Paths read as they appear in the repository — the source's own
        // subpath included, so the reader finds the folder where it says.
        assert_eq!(
            f.path.as_deref(),
            Some("regulation/nl/waterschaps_verordening/hoogland/keur_alpha")
        );
        assert_eq!(f.law_id.as_deref(), Some("keur_alpha_hoogland"));
        // The remedy names the exact new folder path, not just "rename it".
        assert!(
            f.remedy
                .contains("regulation/nl/waterschaps_verordening/hoogland/keur_alpha_hoogland"),
            "{}",
            f.remedy
        );
    }

    // --- check 2: file name vs valid_from ---

    #[test]
    fn a_file_named_after_its_valid_from_is_clean() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2024-07-01.yaml",
                healthy("wet_alpha", "WET", "2024-07-01"),
            )],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_file_named_differently_from_its_valid_from_is_an_error() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2024-07-01.yaml",
                healthy("wet_alpha", "WET", "2025-01-01"),
            )],
            vec![],
        ));
        assert_eq!(kinds(&findings), vec![FindingKind::FileNameMismatch]);
        assert!(
            findings[0]
                .remedy
                .contains("regulation/nl/wet/wet_alpha/2025-01-01.yaml"),
            "{}",
            findings[0].remedy
        );
    }

    #[test]
    fn an_absent_or_referenced_valid_from_is_not_compared() {
        // Absent (the schema doesn't require it) and `#`-referenced (resolved
        // at run time) both have no literal date to compare against.
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2024-07-01.yaml",
                    LawFacts {
                        valid_from: None,
                        ..healthy("wet_alpha", "WET", "")
                    },
                ),
                law(
                    "wet/wet_beta/2024-07-01.yaml",
                    healthy("wet_beta", "WET", "#inwerkingtreding"),
                ),
            ],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    // --- check 3: layer directory vs regulatory_layer ---

    #[test]
    fn a_layer_directory_matching_the_body_is_clean_case_insensitively() {
        let findings = run_checks(&scan(
            vec![law(
                "gemeentelijke_verordening/hoogstad/apv_alpha/2025-01-01.yaml",
                healthy("apv_alpha", "GEMEENTELIJKE_VERORDENING", "2025-01-01"),
            )],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_law_in_the_wrong_layer_directory_is_an_error_once_per_directory() {
        // Two versions in the same misplaced folder are one move, one finding.
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/beleid_alpha/2024-01-01.yaml",
                    healthy("beleid_alpha", "BELEIDSREGEL", "2024-01-01"),
                ),
                law(
                    "wet/beleid_alpha/2025-01-01.yaml",
                    healthy("beleid_alpha", "BELEIDSREGEL", "2025-01-01"),
                ),
            ],
            vec![],
        ));
        assert_eq!(kinds(&findings), vec![FindingKind::LayerDirectoryMismatch]);
        assert!(
            findings[0]
                .remedy
                .contains("regulation/nl/beleidsregel/beleid_alpha"),
            "{}",
            findings[0].remedy
        );
    }

    // --- check 4: duplicate $id ---

    #[test]
    fn distinct_ids_across_directories_are_clean() {
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    healthy("wet_alpha", "WET", "2025-01-01"),
                ),
                law(
                    "wet/wet_beta/2025-01-01.yaml",
                    healthy("wet_beta", "WET", "2025-01-01"),
                ),
            ],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn the_same_id_in_two_directories_is_an_error() {
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    healthy("wet_alpha", "WET", "2025-01-01"),
                ),
                law(
                    "wet/wet_alpha_kopie/2025-01-01.yaml",
                    healthy("wet_alpha", "WET", "2025-01-01"),
                ),
            ],
            vec![],
        ));
        // The copy also trips check 1 (its folder name isn't its id); the
        // duplicate is reported once, naming both folders.
        let dupes = of_kind(&findings, FindingKind::DuplicateLawId);
        assert_eq!(dupes.len(), 1);
        assert!(
            dupes[0]
                .message
                .contains("regulation/nl/wet/wet_alpha_kopie"),
            "{dupes:?}"
        );
        assert!(
            dupes[0].message.contains("regulation/nl/wet/wet_alpha'"),
            "{dupes:?}"
        );
    }

    // --- check 5: cross-law references ---

    #[test]
    fn references_to_existing_laws_are_clean() {
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    LawFacts {
                        references: vec![
                            (LawReferenceKind::SourceRegulation, "wet_beta".to_string()),
                            (LawReferenceKind::Implements, "wet_beta".to_string()),
                        ],
                        ..healthy("wet_alpha", "WET", "2025-01-01")
                    },
                ),
                law(
                    "wet/wet_beta/2025-01-01.yaml",
                    healthy("wet_beta", "WET", "2025-01-01"),
                ),
            ],
            vec![],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_reference_to_an_unknown_id_is_an_error_per_reference() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2025-01-01.yaml",
                LawFacts {
                    references: vec![
                        (LawReferenceKind::SourceRegulation, "wet_zoek".to_string()),
                        (LawReferenceKind::Implements, "wet_weg".to_string()),
                    ],
                    ..healthy("wet_alpha", "WET", "2025-01-01")
                },
            )],
            vec![],
        ));
        assert_eq!(
            kinds(&findings),
            vec![
                FindingKind::UnresolvedLawReference,
                FindingKind::UnresolvedLawReference
            ]
        );
        let targets: Vec<&str> = findings
            .iter()
            .filter_map(|f| f.law_id.as_deref())
            .collect();
        assert_eq!(targets, vec!["wet_alpha", "wet_alpha"]);
        assert!(findings.iter().any(|f| f.message.contains("wet_zoek")));
        assert!(findings.iter().any(|f| f.message.contains("wet_weg")));
    }

    #[test]
    fn a_reference_to_a_law_whose_folder_name_is_wrong_still_resolves() {
        // The scenario from the field: the folder says `keur_alpha`, the body
        // says `keur_alpha_hoogland`. Only check 1 fires — the reference to
        // the declared id is a perfectly good reference, and drowning the
        // cause in consequences would point the reader the wrong way.
        let findings = run_checks(&scan(
            vec![
                law(
                    "waterschaps_verordening/hoogland/keur_alpha/2025-01-01.yaml",
                    healthy(
                        "keur_alpha_hoogland",
                        "WATERSCHAPS_VERORDENING",
                        "2025-01-01",
                    ),
                ),
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    LawFacts {
                        references: vec![(
                            LawReferenceKind::SourceRegulation,
                            "keur_alpha_hoogland".to_string(),
                        )],
                        ..healthy("wet_alpha", "WET", "2025-01-01")
                    },
                ),
            ],
            vec![],
        ));
        assert_eq!(kinds(&findings), vec![FindingKind::DirectoryNameMismatch]);
    }

    // --- check 6: scenario references ---

    #[test]
    fn scenario_steps_naming_known_laws_are_clean() {
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    healthy("wet_alpha", "WET", "2025-01-01"),
                ),
                law(
                    "wet/wet_beta/2025-01-01.yaml",
                    healthy("wet_beta", "WET", "2025-01-01"),
                ),
            ],
            vec![scenario(
                "wet/wet_alpha/scenarios/basis.feature",
                &["wet_alpha"],
                &["wet_beta"],
            )],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_scenario_step_naming_an_unknown_law_is_an_error() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2025-01-01.yaml",
                healthy("wet_alpha", "WET", "2025-01-01"),
            )],
            vec![scenario(
                "wet/wet_alpha/scenarios/basis.feature",
                &["wet_alpha"],
                &["wet_verdwenen"],
            )],
        ));
        assert_eq!(
            kinds(&findings),
            vec![FindingKind::UnresolvedScenarioReference]
        );
        assert_eq!(findings[0].law_id.as_deref(), Some("wet_verdwenen"));
    }

    // --- check 7: a scenarios folder should test its neighbour ---

    #[test]
    fn a_scenario_folder_evaluating_its_neighbour_is_clean() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2025-01-01.yaml",
                healthy("wet_alpha", "WET", "2025-01-01"),
            )],
            vec![
                scenario("wet/wet_alpha/scenarios/een.feature", &[], &[]),
                scenario("wet/wet_alpha/scenarios/twee.feature", &["wet_alpha"], &[]),
            ],
        ));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_scenario_folder_evaluating_only_other_laws_is_a_warning() {
        let findings = run_checks(&scan(
            vec![
                law(
                    "wet/wet_alpha/2025-01-01.yaml",
                    healthy("wet_alpha", "WET", "2025-01-01"),
                ),
                law(
                    "wet/wet_beta/2025-01-01.yaml",
                    healthy("wet_beta", "WET", "2025-01-01"),
                ),
            ],
            vec![scenario(
                "wet/wet_alpha/scenarios/basis.feature",
                &["wet_beta"],
                &[],
            )],
        ));
        assert_eq!(
            kinds(&findings),
            vec![FindingKind::ScenarioDirectoryWithoutTarget]
        );
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(
            findings[0].path.as_deref(),
            Some("regulation/nl/wet/wet_alpha/scenarios")
        );
    }

    #[test]
    fn a_scenario_folder_next_to_a_renamed_law_targets_the_declared_id() {
        // Folder `keur_alpha`, body `$id: keur_alpha_hoogland`, scenario
        // evaluating the declared id: check 7 stays green (the scenario does
        // test its neighbour) while check 1 reports the rename — the exact
        // combination seen in the field.
        let findings = run_checks(&scan(
            vec![law(
                "waterschaps_verordening/hoogland/keur_alpha/2025-01-01.yaml",
                healthy(
                    "keur_alpha_hoogland",
                    "WATERSCHAPS_VERORDENING",
                    "2025-01-01",
                ),
            )],
            vec![scenario(
                "waterschaps_verordening/hoogland/keur_alpha/scenarios/basis.feature",
                &["keur_alpha_hoogland"],
                &[],
            )],
        ));
        assert_eq!(kinds(&findings), vec![FindingKind::DirectoryNameMismatch]);
    }

    // --- unreadable files ---

    #[test]
    fn an_unreadable_file_is_its_own_finding_and_the_rest_still_runs() {
        let mut s = scan(
            vec![law(
                "wet/wet_beta/2024-01-01.yaml",
                healthy("wet_beta", "WET", "2025-01-01"),
            )],
            vec![],
        );
        s.laws.push(ScannedFile {
            relative_path: "wet/wet_alpha/2025-01-01.yaml".to_string(),
            facts: Err("HTTP 403".to_string()),
        });
        let findings = run_checks(&s);
        assert_eq!(
            kinds(&findings),
            vec![FindingKind::FileNameMismatch, FindingKind::FileUnreadable]
        );
        assert!(findings[1].message.contains("HTTP 403"), "{findings:?}");
        assert_eq!(
            findings[1].path.as_deref(),
            Some("regulation/nl/wet/wet_alpha/2025-01-01.yaml")
        );
    }

    // --- ordering & serialisation ---

    #[test]
    fn errors_sort_above_warnings() {
        let findings = run_checks(&scan(
            vec![law(
                "wet/wet_alpha/2024-01-01.yaml",
                healthy("wet_omega", "WET", "2024-01-01"),
            )],
            vec![scenario("wet/wet_alpha/scenarios/basis.feature", &[], &[])],
        ));
        assert_eq!(
            kinds(&findings),
            vec![
                FindingKind::DirectoryNameMismatch,
                FindingKind::ScenarioDirectoryWithoutTarget
            ]
        );
    }

    #[test]
    fn severity_and_kind_serialise_as_stable_keys() {
        let json = serde_json::to_value(Finding {
            severity: Severity::Warning,
            kind: FindingKind::ScenarioDirectoryWithoutTarget,
            path: None,
            law_id: None,
            message: "m".to_string(),
            remedy: "r".to_string(),
        })
        .expect("serialises");
        assert_eq!(json["severity"], "warning");
        assert_eq!(json["kind"], "scenario_directory_without_target");
        // Absent optionals stay out of the payload entirely.
        assert!(json.get("path").is_none());
    }

    #[test]
    fn a_clean_corpus_yields_an_empty_list_not_an_error() {
        assert!(run_checks(&CorpusScan::default()).is_empty());
    }

    // --- facts extraction ---

    #[test]
    fn law_facts_come_from_the_body_only() {
        let facts = LawFacts::from_yaml(
            r#"$id: wet_alpha
regulatory_layer: WET
valid_from: '2025-01-01'
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: bedrag
            source:
              regulation: wet_beta
              output: bedrag
      implements:
        - law: wet_gamma
          article: '2'
          open_term: tarief
"#,
        );
        assert_eq!(facts.declared_id.as_deref(), Some("wet_alpha"));
        assert_eq!(facts.regulatory_layer.as_deref(), Some("WET"));
        assert_eq!(facts.valid_from.as_deref(), Some("2025-01-01"));
        assert_eq!(
            facts.references,
            vec![
                (LawReferenceKind::Implements, "wet_gamma".to_string()),
                (LawReferenceKind::SourceRegulation, "wet_beta".to_string()),
            ]
        );
    }

    #[test]
    fn scenario_facts_split_evaluated_from_loaded_laws() {
        let facts = ScenarioFacts::from_feature(
            r#"Feature: Alpha

  Background:
    Given law "wet_beta" is loaded

  Scenario: Basis
    Then I evaluate "bedrag" of "wet_alpha"

  Scenario: Meerdere uitkomsten
    When I evaluate outputs "bedrag, recht" of "wet_omega"
"#,
        );
        // Both evaluation steps count as targets — a folder whose scenarios
        // only use the multi-output form still tests its law.
        assert_eq!(facts.evaluated_ids, vec!["wet_alpha", "wet_omega"]);
        assert_eq!(facts.loaded_ids, vec!["wet_beta"]);
    }

    #[test]
    fn short_error_keeps_messages_inside_a_sentence() {
        assert_eq!(short_error("kort"), "kort");
        assert_eq!(short_error("twee\nregels"), "twee regels");
        let long = short_error(&"x".repeat(500));
        assert_eq!(long.chars().count(), 120);
        assert!(long.ends_with('…'));
    }
}
