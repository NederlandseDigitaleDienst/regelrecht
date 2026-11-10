//! Validate note sidecar files (RFC-005, RFC-018).
//!
//! For each note file:
//! 1. JSON Schema validation against the embedded annotation schema.
//! 2. Resolve every note's selector against its target law (loaded from the
//!    corpus by `$id`). Orphaned or ambiguous notes are reported as
//!    **warnings**, not errors (RFC-018 Decision 8): law text legitimately
//!    drifts away from notes over time.
//! 3. Tagging-body values are checked against the controlled vocabulary
//!    (`corpus/annotations/_vocabulary/ambiguity.yaml`); unknown values are
//!    **warnings** (RFC-018 Decision 9).
//!
//! Exit code is non-zero only on schema validation failures, or (exit 2)
//! when the corpus or the embedded schema cannot be found at all — a run
//! that never saw the corpus must not look like a clean one.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process;

use jsonschema::Validator;
use regelrecht_engine::annotation::{SkipReason, TextQuoteSelector, law_id_from_source, resolve};
use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};

const ANNOTATION_SCHEMA: &str = include_str!("../../../../schema/v0.5.3/annotation-schema.json");

fn main() {
    let root = match resolve_repo_root(
        std::env::var("REGELRECHT_REPO_ROOT").ok().as_deref(),
        std::env::current_dir().ok().as_deref(),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    ) {
        Some(root) => root,
        None => {
            eprintln!(
                "FATAL: repo root not found (no corpus/annotations directory); \
                 run from inside the repo or set REGELRECHT_REPO_ROOT"
            );
            process::exit(2);
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<PathBuf> = if args.is_empty() {
        discover_note_files(&root)
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    if files.is_empty() {
        eprintln!("No note files found.");
        return;
    }

    let schema: serde_json::Value = match serde_json::from_str(ANNOTATION_SCHEMA) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FATAL: embedded annotation schema is not valid JSON: {e}");
            process::exit(2);
        }
    };
    let validator = match Validator::new(&schema) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FATAL: annotation schema does not compile: {e}");
            process::exit(2);
        }
    };

    let vocabulary = load_vocabulary(&root);
    let mut failed = false;
    let mut warnings = 0usize;

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FAIL: {}: read: {e}", path.display());
                failed = true;
                continue;
            }
        };
        let doc: serde_json::Value = match serde_yaml_ng::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("FAIL: {}: yaml parse: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let errors: Vec<_> = validator.iter_errors(&doc).collect();
        if !errors.is_empty() {
            eprintln!("FAIL: {}: schema", path.display());
            for err in &errors {
                eprintln!("  - {}: {}", err.instance_path(), err);
            }
            failed = true;
            continue;
        }
        eprintln!("OK: {} (annotation schema v0.5.3)", path.display());

        warnings += check_notes(path, &doc, &vocabulary, &root);
    }

    if warnings > 0 {
        eprintln!(
            "\n{warnings} warning(s). Orphaned/ambiguous notes and unknown tags do not fail the build (RFC-018)."
        );
    }
    if failed {
        process::exit(1);
    }
}

/// Why a resolve was skipped, in the words the author needs.
///
/// Skipped is not the same as orphaned: the resolver never (fully) searched,
/// so absence was not established. Only a quote-length skip is fixed by
/// shortening the quote; a scan or scoring budget that ran out has nothing to
/// do with this quote, and saying otherwise sends the author after a change
/// that cannot help.
fn skip_cause(reason: Option<SkipReason>, quote_chars: usize) -> String {
    match reason {
        Some(SkipReason::QuoteTooLong) => {
            format!("quote of {quote_chars} chars exceeds the fuzzy quote cap; shorten the quote")
        }
        _ => "the law exceeds the fuzzy scan budget; the text was not fully searched".to_string(),
    }
}

/// Resolve each note and check tag values; return the warning count.
fn check_notes(
    path: &Path,
    doc: &serde_json::Value,
    vocabulary: &HashSet<String>,
    root: &Path,
) -> usize {
    let Some(notes) = doc.get("annotations").and_then(|v| v.as_array()) else {
        return 0;
    };

    let mut warnings = 0;
    for (i, note) in notes.iter().enumerate() {
        // Resolve the selector against the target law. A law that cannot be
        // loaded is a warning, not a silent skip: otherwise step 2 quietly
        // never ran for this note and "OK" overstates what was checked.
        if let Some(law_id) = note
            .get("target")
            .and_then(|t| t.get("source"))
            .and_then(|s| s.as_str())
            .and_then(law_id_from_source)
        {
            match load_law_by_id(root, law_id) {
                Ok(law) => {
                    if let Some(selector) = note
                        .get("target")
                        .and_then(|t| t.get("selector"))
                        .and_then(|s| serde_json::from_value::<TextQuoteSelector>(s.clone()).ok())
                    {
                        let result = resolve(&selector, &law.articles);
                        if result.is_orphaned() {
                            eprintln!(
                                "  WARN: {} note[{i}]: orphaned (selector {:?} not found in law)",
                                path.display(),
                                selector.exact
                            );
                            warnings += 1;
                        } else if result.is_ambiguous() {
                            eprintln!(
                                "  WARN: {} note[{i}]: ambiguous ({} matches for {:?}; add prefix/suffix)",
                                path.display(),
                                result.matches.len(),
                                selector.exact
                            );
                            warnings += 1;
                        } else if result.is_skipped() {
                            let cause =
                                skip_cause(result.skip_reason, selector.exact.chars().count());
                            eprintln!(
                                "  WARN: {} note[{i}]: not searched ({cause})",
                                path.display()
                            );
                            warnings += 1;
                        }
                    }
                }
                Err(failure) => {
                    let detail = match failure {
                        LawLoadFailure::NotFound => {
                            format!("law '{law_id}' not found in corpus")
                        }
                        LawLoadFailure::Unreadable(e) => {
                            format!("law '{law_id}' cannot be loaded: {e}")
                        }
                    };
                    eprintln!(
                        "  WARN: {} note[{i}]: selector not checked ({detail})",
                        path.display()
                    );
                    warnings += 1;
                }
            }
        }

        // Check tagging-body values against the controlled vocabulary.
        for tag in tagging_values(note) {
            if !vocabulary.contains(&tag) {
                eprintln!(
                    "  WARN: {} note[{i}]: tag {tag:?} not in _vocabulary/ambiguity.yaml",
                    path.display()
                );
                warnings += 1;
            }
        }
    }
    warnings
}

/// Collect every `TextualBody` value whose `purpose` is `tagging`.
fn tagging_values(note: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let bodies = match note.get("body") {
        Some(serde_json::Value::Array(a)) => a.clone(),
        Some(other) => vec![other.clone()],
        None => return out,
    };
    for body in bodies {
        let is_tag = body.get("purpose").and_then(|p| p.as_str()) == Some("tagging");
        if is_tag && let Some(v) = body.get("value").and_then(|v| v.as_str()) {
            out.push(v.to_string());
        }
    }
    out
}

/// Locate the repo root at runtime. A directory only counts as root when it
/// actually holds `corpus/annotations`; a baked-in build-machine path would
/// otherwise make a run elsewhere scan nothing and still report success.
///
/// Order: an explicit override (`REGELRECHT_REPO_ROOT`), then walking up from
/// the current directory, then the compile-time manifest dir (which only
/// exists on the machine that built the binary). An override that does not
/// hold a corpus is a misconfiguration and does not fall through.
fn resolve_repo_root(
    override_root: Option<&str>,
    cwd: Option<&Path>,
    manifest_dir: &Path,
) -> Option<PathBuf> {
    fn has_corpus(dir: &Path) -> bool {
        dir.join("corpus").join("annotations").is_dir()
    }

    if let Some(root) = override_root {
        let root = PathBuf::from(root);
        return has_corpus(&root).then_some(root);
    }
    if let Some(start) = cwd {
        let mut dir = start;
        loop {
            if has_corpus(dir) {
                return Some(dir.to_path_buf());
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }
    let fallback = manifest_dir.join("..").join("..");
    has_corpus(&fallback).then_some(fallback)
}

/// All `corpus/annotations/**/annotations.yaml` files (skips `_vocabulary`).
fn discover_note_files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join("corpus/annotations");
    let mut out = Vec::new();
    collect_yaml(&dir, &mut out);
    out.sort();
    out
}

fn collect_yaml(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().and_then(|n| n.to_str()) == Some("_vocabulary") {
                continue;
            }
            collect_yaml(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("yaml") {
            out.push(p);
        }
    }
}

/// Why a law referenced by a note could not be produced for selector checks.
#[derive(Debug)]
enum LawLoadFailure {
    /// No corpus YAML carries this `$id`.
    NotFound,
    /// A matching YAML exists but does not load as a law. Distinct from
    /// `NotFound`: the note is not orphaned, the law file itself is the
    /// problem, and that must not be silenced.
    Unreadable(String),
}

/// Load the latest version of a law identified by its `$id` from the corpus.
///
/// Scans `corpus/regulation/` for a YAML whose `$id` matches, preferring the
/// lexicographically last filename (latest `valid_from`).
fn load_law_by_id(root: &Path, law_id: &str) -> Result<ArticleBasedLaw, LawLoadFailure> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    collect_law_yaml(&root.join("corpus/regulation"), law_id, &mut candidates);
    candidates.sort();
    let path = candidates.last().ok_or(LawLoadFailure::NotFound)?;
    ArticleBasedLaw::from_yaml_file(path)
        .map_err(|e| LawLoadFailure::Unreadable(format!("{}: {e}", path.display())))
}

fn collect_law_yaml(dir: &Path, law_id: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_law_yaml(&p, law_id, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("yaml") {
            // Parse the top-level `$id` rather than substring-matching the
            // file: a comment or a nested string containing "$id: x" would
            // otherwise produce a false positive.
            if let Ok(content) = std::fs::read_to_string(&p)
                && let Ok(doc) = serde_yaml_ng::from_str::<serde_json::Value>(&content)
                && doc.get("$id").and_then(|v| v.as_str()) == Some(law_id)
            {
                out.push(p);
            }
        }
    }
}

/// Load the ambiguity vocabulary `id`s. Missing file means an empty set
/// (every tag will warn), which surfaces the misconfiguration.
fn load_vocabulary(root: &Path) -> HashSet<String> {
    let path = root.join("corpus/annotations/_vocabulary/ambiguity.yaml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!("WARN: vocabulary not found at {}", path.display());
        return HashSet::new();
    };
    let doc: serde_json::Value = match serde_yaml_ng::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("WARN: vocabulary parse error: {e}");
            return HashSet::new();
        }
    };
    doc.get("ambiguity")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|it| it.get("id").and_then(|v| v.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vocabulary_of(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| (*s).to_string()).collect()
    }

    /// The repo root as seen from the build tree, for tests that read the
    /// real corpus.
    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// A throwaway corpus root with empty `corpus/annotations` and
    /// `corpus/regulation` directories, removed on drop.
    struct TempRoot(PathBuf);

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_root(name: &str) -> TempRoot {
        let root = temp_dir_without_corpus(name);
        std::fs::create_dir_all(root.0.join("corpus/annotations")).unwrap();
        std::fs::create_dir_all(root.0.join("corpus/regulation")).unwrap();
        root
    }

    /// A throwaway directory that holds no corpus at all (and, being under
    /// the temp dir, has no ancestor holding one either).
    fn temp_dir_without_corpus(name: &str) -> TempRoot {
        let dir = std::env::temp_dir().join(format!(
            "regelrecht-validate-annotations-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempRoot(dir)
    }

    fn tag_note(tag: &str) -> serde_json::Value {
        json!({
            "body": {"type": "TextualBody", "value": tag, "purpose": "tagging"}
        })
    }

    /// A note anchored to a law that is in the corpus, so that the selector is
    /// really resolved instead of silently skipped.
    fn zorgtoeslag_note(exact: &str) -> serde_json::Value {
        json!({
            "target": {
                "source": "regelrecht://wet_op_de_zorgtoeslag",
                "selector": {"type": "TextQuoteSelector", "exact": exact}
            }
        })
    }

    #[test]
    fn tagging_value_is_read_from_a_single_body() {
        assert_eq!(
            tagging_values(&tag_note("open-norm-partial")),
            vec!["open-norm-partial".to_string()]
        );
    }

    #[test]
    fn every_tagging_body_in_a_list_is_read() {
        let note = json!({"body": [
            {"type": "TextualBody", "value": "open-norm-partial", "purpose": "tagging"},
            {"type": "TextualBody", "value": "een toelichting", "purpose": "commenting"},
            {"type": "TextualBody", "value": "missing-document", "purpose": "tagging"},
        ]});
        assert_eq!(
            tagging_values(&note),
            vec![
                "open-norm-partial".to_string(),
                "missing-document".to_string()
            ]
        );
    }

    #[test]
    fn a_body_without_the_tagging_purpose_is_no_tag() {
        let note = json!({
            "body": {"type": "TextualBody", "value": "een toelichting", "purpose": "commenting"}
        });
        assert!(tagging_values(&note).is_empty());
        assert!(tagging_values(&json!({})).is_empty());
    }

    #[test]
    fn every_tag_outside_the_vocabulary_gives_one_warning() {
        let doc = json!({"annotations": [tag_note("staat-er-niet"), tag_note("ook-niet")]});
        assert_eq!(
            check_notes(
                Path::new("notes.yaml"),
                &doc,
                &vocabulary_of(&["open-norm-partial"]),
                &repo()
            ),
            2
        );
    }

    #[test]
    fn a_tag_from_the_vocabulary_gives_no_warning() {
        let doc = json!({"annotations": [tag_note("open-norm-partial")]});
        assert_eq!(
            check_notes(
                Path::new("notes.yaml"),
                &doc,
                &vocabulary_of(&["open-norm-partial"]),
                &repo()
            ),
            0
        );
    }

    #[test]
    fn a_file_without_annotations_gives_no_warning() {
        assert_eq!(
            check_notes(
                Path::new("notes.yaml"),
                &json!({}),
                &vocabulary_of(&[]),
                &repo()
            ),
            0
        );
    }

    /// A term from another area of law: no article of the Wet op de
    /// zorgtoeslag contains it, and it is far enough from anything in the text
    /// that the fuzzy matcher finds no candidate either.
    #[test]
    fn a_selector_that_no_longer_occurs_warns_as_orphaned() {
        let doc = json!({"annotations": [zorgtoeslag_note("motorrijtuigenbelasting")]});
        assert_eq!(
            check_notes(Path::new("notes.yaml"), &doc, &vocabulary_of(&[]), &repo()),
            1
        );
    }

    /// A quote too long for the bounded fuzzy scan: the resolver reports the
    /// search as skipped, and the validator must warn "not searched" rather
    /// than stay silent (silence would read as "resolves fine").
    #[test]
    fn a_selector_too_long_to_search_warns_as_not_searched() {
        let quote = "motorrijtuigenbelasting ".repeat(6);
        assert!(quote.chars().count() > regelrecht_engine::config::MAX_FUZZY_QUOTE_CHARS);
        let doc = json!({"annotations": [zorgtoeslag_note(&quote)]});
        assert_eq!(
            check_notes(Path::new("notes.yaml"), &doc, &vocabulary_of(&[]), &repo()),
            1
        );
    }

    /// The warning must name the bound that was actually hit. "Shorten the
    /// quote" is advice for one of the three causes only; on the other two it
    /// sends the author after a change that cannot help.
    #[test]
    fn the_skip_warning_only_blames_the_quote_when_the_quote_was_the_cause() {
        let long = skip_cause(Some(SkipReason::QuoteTooLong), 240);
        assert!(long.contains("240 chars"), "{long}");
        assert!(long.contains("shorten the quote"), "{long}");

        let budget = skip_cause(Some(SkipReason::SearchBudget), 40);
        assert!(!budget.contains("shorten the quote"), "{budget}");
        assert!(!budget.contains("40"), "{budget}");
        assert!(budget.contains("not fully searched"), "{budget}");
    }

    #[test]
    fn a_selector_that_occurs_more_than_once_warns_as_ambiguous() {
        let doc = json!({"annotations": [zorgtoeslag_note("verzekerde")]});
        assert_eq!(
            check_notes(Path::new("notes.yaml"), &doc, &vocabulary_of(&[]), &repo()),
            1
        );
    }

    #[test]
    fn a_law_is_found_by_its_id_in_the_corpus() {
        let law = load_law_by_id(&repo(), "wet_op_de_zorgtoeslag").expect("law is in the corpus");
        assert_eq!(law.id, "wet_op_de_zorgtoeslag");
        assert!(!law.articles.is_empty());
    }

    #[test]
    fn an_unknown_law_id_reports_not_found() {
        assert!(matches!(
            load_law_by_id(&repo(), "wet_die_niet_bestaat"),
            Err(LawLoadFailure::NotFound)
        ));
    }

    /// A corpus YAML that carries the right `$id` but does not load as a law
    /// (here: `articles` is not a list, as happens when the law format runs
    /// ahead of the law model). The old code swallowed this with `.ok()`.
    #[test]
    fn a_law_that_fails_to_load_reports_unreadable_not_not_found() {
        let root = temp_root("unreadable-law");
        std::fs::write(
            root.0.join("corpus/regulation/2025-01-01.yaml"),
            "$id: broken_law\narticles: dit-is-geen-lijst\n",
        )
        .unwrap();

        assert!(matches!(
            load_law_by_id(&root.0, "broken_law"),
            Err(LawLoadFailure::Unreadable(_))
        ));
    }

    /// The premise of the silent-skip bug: the note points at a law whose
    /// YAML is present and matched by `$id`, but the law model cannot load
    /// it. Step 2 (selector resolution) then never runs, and that must be a
    /// warning — not zero output followed by "OK".
    #[test]
    fn a_note_whose_law_cannot_be_loaded_warns_instead_of_passing_silently() {
        let root = temp_root("silent-skip");
        std::fs::write(
            root.0.join("corpus/regulation/2025-01-01.yaml"),
            "$id: broken_law\narticles: dit-is-geen-lijst\n",
        )
        .unwrap();

        let doc = json!({"annotations": [{
            "target": {
                "source": "regelrecht://broken_law",
                "selector": {"type": "TextQuoteSelector", "exact": "iets"}
            }
        }]});
        assert_eq!(
            check_notes(Path::new("notes.yaml"), &doc, &vocabulary_of(&[]), &root.0),
            1
        );
    }

    /// A note pointing at a law that is nowhere in the corpus is equally a
    /// skipped selector check, and warns.
    #[test]
    fn a_note_whose_law_is_absent_from_the_corpus_warns() {
        let root = temp_root("absent-law");
        let doc = json!({"annotations": [{
            "target": {
                "source": "regelrecht://wet_die_niet_bestaat",
                "selector": {"type": "TextQuoteSelector", "exact": "iets"}
            }
        }]});
        assert_eq!(
            check_notes(Path::new("notes.yaml"), &doc, &vocabulary_of(&[]), &root.0),
            1
        );
    }

    #[test]
    fn note_discovery_finds_the_sidecars_and_skips_the_vocabulary() {
        let files = discover_note_files(&repo());
        assert!(
            !files.is_empty(),
            "corpus/annotations holds at least one note file"
        );
        for file in &files {
            assert_eq!(file.extension().and_then(|e| e.to_str()), Some("yaml"));
            assert!(
                !file.components().any(|c| c.as_os_str() == "_vocabulary"),
                "{} is vocabulary, not a note file",
                file.display()
            );
        }
    }

    #[test]
    fn the_vocabulary_holds_the_ambiguity_ids() {
        let vocabulary = load_vocabulary(&repo());
        assert!(vocabulary.contains("open-norm-not-filled"));
        assert!(vocabulary.contains("needs-uitvoeringsbeleid"));
    }

    #[test]
    fn the_repo_root_is_found_by_walking_up_from_a_subdirectory() {
        let repo = repo().canonicalize().unwrap();
        let start = repo.join("packages").join("engine").join("src");
        let found = resolve_repo_root(None, Some(&start), Path::new("/nonexistent"))
            .expect("walking up from packages/engine/src reaches the repo root");
        assert_eq!(found.canonicalize().unwrap(), repo);
    }

    /// An explicit override wins, and must itself hold a corpus: a wrong
    /// override must surface as "no root", not silently fall through to
    /// whatever the current directory happens to contain.
    #[test]
    fn an_explicit_override_must_itself_hold_a_corpus() {
        let repo = repo();
        let found = resolve_repo_root(
            Some(repo.to_str().unwrap()),
            None,
            Path::new("/nonexistent"),
        );
        assert!(found.is_some(), "a valid override is accepted as-is");

        let root = temp_dir_without_corpus("no-corpus-override");
        assert!(
            resolve_repo_root(
                Some(root.0.to_str().unwrap()),
                Some(&repo),
                &repo.join("packages").join("engine"),
            )
            .is_none(),
            "an override without a corpus does not fall through to cwd or manifest"
        );
    }

    /// Away from the repo, the compile-time manifest dir still works on the
    /// build machine — and on any other machine, where that path does not
    /// exist, the outcome is honestly "no root" instead of a clean-looking
    /// run over zero files.
    #[test]
    fn away_from_the_repo_only_an_existing_manifest_fallback_helps() {
        let root = temp_dir_without_corpus("far-away-cwd");
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let found = resolve_repo_root(None, Some(&root.0), &manifest)
            .expect("the build machine still has the manifest dir");
        assert_eq!(
            found.canonicalize().unwrap(),
            repo().canonicalize().unwrap()
        );

        assert!(
            resolve_repo_root(
                None,
                Some(&root.0),
                Path::new("/nonexistent/packages/engine")
            )
            .is_none(),
            "elsewhere the binary must not pretend it saw a corpus"
        );
    }
}
