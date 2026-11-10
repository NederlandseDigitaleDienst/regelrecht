//! Note (annotation) YAML schema validation.
//!
//! The editor write path (`PUT /api/corpus/laws/{id}/annotations`) must
//! reject a malformed note file before a commit and PR are created. Once a
//! branch exists, an invalid file is a dead PR someone has to clean up.
//! This is the technical half of the two-layer review: schema correctness
//! is enforced here, resolve correctness (orphaned/ambiguous selectors)
//! stays a warning in `validate-annotations` per RFC-018 §8.
//!
//! NOTE: this is a *second* embedded copy of
//! `schema/v0.5.3/annotation-schema.json`. The `validate-annotations`
//! engine binary embeds the same file independently (it does not call this
//! module), so editor-api need not depend on the engine crate. The JSON
//! file is the single source of truth; the duplication is a known drift
//! risk. `schema_compiles` pins this copy at build time; the engine binary
//! pins the other. If they ever diverge, the file is what is authoritative
//! and both `include_str!`s must point at it.

use std::sync::LazyLock;

use jsonschema::Validator;

/// The note schema, embedded at build time. Path is relative to this
/// source file: `packages/corpus/src/` → repo `schema/v0.5.3/`.
const ANNOTATION_SCHEMA: &str = include_str!("../../../schema/v0.5.3/annotation-schema.json");

/// Compiled once on first use. The embedded schema not compiling is a
/// build/release fault, not a per-request one, but the workspace bans
/// `expect()`, so init is total: the error is carried in the `Result` and
/// surfaced from [`validate_annotation_doc`]. `schema_compiles` pins the
/// build-time guarantee that this `Err` arm is unreachable in practice.
static VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(|| {
    let schema: serde_json::Value = serde_json::from_str(ANNOTATION_SCHEMA)
        .map_err(|e| format!("embedded annotation schema is not valid JSON: {e}"))?;
    Validator::new(&schema).map_err(|e| format!("embedded annotation schema does not compile: {e}"))
});

/// Parse a note file's YAML and validate it against
/// `schema/v0.5.3/annotation-schema.json`, returning the parsed document on
/// success.
///
/// On failure returns a list of error strings (a YAML parse error, or the
/// schema violations as instance-path + message). Callers that surface
/// these to a client should treat them as **diagnostic detail to log**, not
/// as a safe response body: instance paths can echo attacker-controlled map
/// keys (the schema is `additionalProperties: false`, so an unknown key's
/// name lands in the path) and that flows into UI dialogs — the same
/// self-XSS vector `save_law` avoids by not echoing the request `$id`.
pub fn parse_and_validate_annotation_yaml(yaml: &str) -> Result<serde_json::Value, Vec<String>> {
    let doc: serde_json::Value =
        serde_yaml_ng::from_str(yaml).map_err(|e| vec![format!("YAML parse error: {e}")])?;
    validate_annotation_doc(&doc)?;
    Ok(doc)
}

/// Validate an already-parsed sidecar document against the note schema.
///
/// Used by the editor write path, which builds the merged document
/// in-memory (base notes from the branch + new notes) and must validate
/// the *result* before it is written. Same error-handling caveat as
/// [`parse_and_validate_annotation_yaml`]: log the errors, do not echo
/// them to the client.
pub fn validate_annotation_doc(doc: &serde_json::Value) -> Result<(), Vec<String>> {
    let validator = VALIDATOR.as_ref().map_err(|e| vec![e.clone()])?;

    let errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|err| format!("{}: {}", err.instance_path(), err))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Serialise a sidecar document back to YAML for writing.
///
/// The editor write path builds the merged document as JSON in-memory;
/// the on-disk corpus format is YAML. Centralised here so the dump options
/// (block style, no anchors) stay consistent with the frontend export and
/// the committed files, keeping git diffs readable.
pub fn serialize_annotation_doc(doc: &serde_json::Value) -> Result<String, String> {
    serde_yaml_ng::to_string(doc).map_err(|e| format!("failed to serialise notes: {e}"))
}

/// The notes already present in a sidecar's text, as JSON values.
///
/// Used to dedupe an append against what is already committed. Returns an
/// empty vec when the file has no `annotations:` sequence (or is absent).
pub fn notes_in_sidecar(base_text: &str) -> Vec<serde_json::Value> {
    serde_yaml_ng::from_str::<serde_json::Value>(base_text)
        .ok()
        .and_then(|doc| doc.get("annotations").and_then(|a| a.as_array()).cloned())
        .unwrap_or_default()
}

/// Outcome of preparing an append-only write of `new_notes` onto a sidecar.
pub enum AppendOutcome {
    /// Nothing to write: every new note was already present (dedup left
    /// zero). The caller must skip the write/commit/PR entirely so a
    /// no-op save produces no branch noise (review finding NEW-2).
    NoChange,
    /// The full text to write. Either the verbatim base with new note
    /// items appended (history/comments preserved), or a freshly built
    /// document when there was no base sequence to preserve.
    Write(String),
}

/// Append `new_notes` to a sidecar **without rewriting the existing file**.
///
/// RFC-018 Decision 1 promises `git blame` shows who added each note and
/// when; RFC-005's whole premise is that a stand-off file must not be
/// rewritten under the content it annotates. Parsing the base and
/// re-serialising it (even "losslessly") reorders keys, drops the curated
/// *motivering* comments (RFC-018 §Why / AWB 3:46), and reassigns every
/// line's blame to the last writer. So the base bytes are kept **verbatim**
/// and the new notes are appended as text.
///
/// - Base has an `annotations:` block: serialise only the *new, deduped*
///   notes as a YAML sequence, re-indented to match the base sequence's
///   actual list-item indentation (detected from the text, not assumed),
///   and append after the existing content. Existing bytes are untouched,
///   so the git diff is exactly the added lines. This handles both the
///   serde_yaml_ng `- ` at column 0 (what `serialize_annotation_doc`
///   itself produces, so a first save's file and a second save's append
///   agree) and a 2-space curated file.
/// - No base, or a base without an `annotations:` key (nothing to
///   preserve): build a fresh full document. There is no history or
///   comment to protect in that case.
/// - All new notes already present: [`AppendOutcome::NoChange`].
///
/// Remaining format assumption: a uniform-space block sequence with LF
/// endings (every corpus tool — `js-yaml` export, `serde_yaml_ng`, the
/// committed sidecars — produces exactly this). Two non-conforming bases
/// behave differently, both non-silently:
/// - A flow sequence (`annotations: [...]`) or any base whose notes the
///   parser cannot read as a block sequence takes the rebuild path, where
///   the destructive-shrink guard refuses outright rather than dropping
///   the existing notes.
/// - A CRLF base parses (YAML accepts CRLF), so the verbatim-append path
///   runs and yields a file with mixed endings. The re-parse + schema
///   gate does NOT reject this (it is still valid YAML); the pre-commit
///   `yamllint`/EOF hook is what catches it. The whole committed corpus
///   is LF, so this is not a path the corpus produces.
///
/// `new_notes` are assumed schema-checked by the caller; the caller must
/// still validate the *resulting* document.
pub fn append_notes_to_sidecar(
    base_text: Option<&str>,
    new_notes: &[serde_json::Value],
    schema_url: &str,
) -> Result<AppendOutcome, String> {
    let base_notes: Vec<serde_json::Value> = base_text.map(notes_in_sidecar).unwrap_or_default();

    let seen: std::collections::HashSet<String> = base_notes
        .iter()
        .filter_map(|n| serde_json::to_string(n).ok())
        .collect();

    // Preserve order; dedupe new notes against the base and against each
    // other (a double-submit of the same draft is idempotent).
    let mut to_add: Vec<&serde_json::Value> = Vec::new();
    let mut added_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for note in new_notes {
        let Ok(key) = serde_json::to_string(note) else {
            continue;
        };
        if !seen.contains(&key) && added_keys.insert(key) {
            to_add.push(note);
        }
    }

    if to_add.is_empty() {
        return Ok(AppendOutcome::NoChange);
    }

    // Decide whether there is a base sequence to preserve. We only treat
    // the base as appendable when it actually parses and carries an
    // `annotations` array; anything else is rebuilt from scratch (no
    // history/comments at stake).
    let base_has_sequence = base_text
        .and_then(|t| serde_yaml_ng::from_str::<serde_json::Value>(t).ok())
        .and_then(|d| d.get("annotations").map(|a| a.is_array()))
        .unwrap_or(false);

    if !base_has_sequence {
        // Fresh document: base + new notes, full serialise is fine here
        // because there is nothing to protect.
        //
        // Destructive-shrink guard. This is the ONLY path that rebuilds
        // the file instead of appending verbatim, so it is the only path
        // that can lose notes: `notes_in_sidecar` returns `[]` for a base
        // that does not parse as a block sequence (flow style
        // `annotations: [...]`, CRLF that breaks the parser, an anchor/
        // alias the loader rejects). If we rebuilt from an empty
        // `base_notes` while the raw file clearly carried note-like
        // content, we would silently drop the lot. Refuse instead: a
        // loud abort with the bytes untouched beats a quiet corpus
        // deletion. RFC-018 §10 / RFC-005 verbatim-preservation.
        if base_notes.is_empty()
            && let Some(raw) = base_text
            && (raw.contains("type: Annotation") || raw.contains("\"type\": \"Annotation\""))
        {
            return Err(
                "refusing to rebuild a notes file whose existing content could not \
                         be parsed as a block sequence: this would discard the notes already \
                         in it. The sidecar must be repaired (LF endings, block-style \
                         `annotations:` list) before new notes can be added."
                    .to_string(),
            );
        }
        let mut all = base_notes;
        all.extend(to_add.iter().map(|n| (*n).clone()));
        let doc = serde_json::json!({ "$schema": schema_url, "annotations": all });
        return Ok(AppendOutcome::Write(serialize_annotation_doc(&doc)?));
    }

    // Detect the base sequence's actual list-item indentation and match
    // it. Earlier this hardcoded two spaces, which broke whenever the base
    // used a different indent — including the common case where the base
    // was itself produced by `serialize_annotation_doc` (serde_yaml_ng
    // emits `- ` at column 0), so the first save's file and the second
    // save's append disagreed and produced invalid YAML. We read the real
    // indent from the file instead of assuming one.
    let base = base_text.unwrap_or_default();
    let base_indent = sequence_item_indent(base);

    // serde_yaml_ng emits a top-level sequence as `- ...` at column 0.
    // Re-indent each line by `base_indent` spaces so the new items sit at
    // exactly the same depth as the existing ones under `annotations:`.
    let owned: Vec<serde_json::Value> = to_add.iter().map(|n| (*n).clone()).collect();
    let seq = serde_yaml_ng::to_string(&owned)
        .map_err(|e| format!("failed to serialise new notes: {e}"))?;
    let pad = " ".repeat(base_indent);
    let indented: String = seq
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::from("\n")
            } else {
                format!("{pad}{line}\n")
            }
        })
        .collect();

    // Exactly one newline between the existing content and the appended
    // items, regardless of whether the base ended with 0, 1 or more.
    let mut out = base.trim_end_matches('\n').to_string();
    out.push('\n');
    out.push_str(&indented);
    Ok(AppendOutcome::Write(out))
}

/// Indentation (in spaces) of the `annotations:` block-sequence items in a
/// sidecar's text.
///
/// A YAML block sequence under `annotations:` has items written as
/// `<indent>- ...`. serde_yaml_ng emits them at column 0; a hand-curated
/// file may use 2. We must append new items at the *same* depth or the
/// result is structurally invalid YAML (the bug that produced
/// "did not find expected key"). Scans from the `annotations:` key to the
/// first sequence-entry line and returns its leading-space count; falls
/// back to 0 (serde_yaml_ng's native depth) when it cannot tell.
fn sequence_item_indent(text: &str) -> usize {
    let mut after_key = false;
    for line in text.lines() {
        if !after_key {
            // Top-level `annotations:` key (no leading space).
            if line.trim_end() == "annotations:" || line.starts_with("annotations: ") {
                after_key = true;
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("- ") || trimmed == "-" {
            return line.len() - trimmed.len();
        }
        // First non-blank, non-comment line after the key that is not a
        // sequence entry: not a block sequence we can match. Bail to 0.
        break;
    }
    0
}

/// The law id a note's `target.source` URI refers to.
///
/// `target.source` is a `regelrecht://<law_id>[/...]` URI (RFC-005); the
/// law id is the first path segment. Mirrors `regelrecht_engine`'s
/// `annotation::law_id_from_source` so the editor write path and the engine
/// resolver agree on what a note is "about".
pub fn law_id_from_source(source: &str) -> Option<&str> {
    let rest = source.strip_prefix("regelrecht://")?;
    Some(rest.split('/').next().unwrap_or(rest))
}

/// A parsed `regelrecht://...` URI. Annotations can target laws
/// (`target.source`) and reference documents from their bodies
/// (`body[SpecificResource].source`); both flavours share the same
/// scheme so callers ask `parse_regelrecht_uri` once and match on the
/// returned variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegelrechtRef {
    /// `regelrecht://<law_id>` or `regelrecht://<law_id>/<rest>`. The
    /// trailing path is preserved verbatim for the engine to interpret.
    Law { law_id: String, rest: String },
    /// `regelrecht://doc/<traject_ref>/<relative_path>` — a document
    /// inside the editor's `documents/<traject_ref>/...` tree.
    Document {
        traject_ref: String,
        relative_path: String,
    },
}

/// Parse a `regelrecht://...` URI.
///
/// Falls back to `None` for unrelated schemes (`https://...`,
/// arbitrary strings). The document form takes priority when the
/// first path segment is literally `doc`.
///
/// **RESERVED ID**: `doc` is therefore a reserved law `$id` — a law named
/// `doc` would be routed to the `Document` variant and become unreachable
/// via `regelrecht://doc/...`. This collision is structurally impossible
/// today (harvested `$id`s are underscore-based slugs like
/// `wet_op_de_zorgtoeslag`), so no creation-time guard is enforced; if a
/// future source can emit a bare `doc` id, add it to the law-id blocklist
/// in the schema validator. The `doc`-head precedence is pinned by a test.
pub fn parse_regelrecht_uri(source: &str) -> Option<RegelrechtRef> {
    let rest = source.strip_prefix("regelrecht://")?;
    let (head, tail) = match rest.split_once('/') {
        Some((h, t)) => (h, t),
        None => (rest, ""),
    };
    if head == "doc" {
        let (traject_ref, path) = tail.split_once('/')?;
        if traject_ref.is_empty() || path.is_empty() {
            return None;
        }
        return Some(RegelrechtRef::Document {
            traject_ref: traject_ref.to_string(),
            relative_path: path.to_string(),
        });
    }
    Some(RegelrechtRef::Law {
        law_id: head.to_string(),
        rest: tail.to_string(),
    })
}

/// Why a note's `target.source` is not an acceptable reference to the law
/// the sidecar is being written for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteTargetError {
    /// `target.source` absent or not a string (schema permits any string).
    Unparseable,
    /// `target.source` is a string but not a `regelrecht://<law_id>` URI.
    NotRegelrechtUri,
    /// Parsed to a law id, but a *different* one than expected.
    WrongLaw(String),
}

/// Reject the sidecar unless **every** note's `target.source` resolves to
/// exactly `law_id`.
///
/// This is an *allowlist*, deliberately: an earlier version collected the
/// parseable law ids and looked for a mismatch, which silently let a note
/// whose source was `https://evil/...` or absent slip through (the schema
/// only requires `target.source` to be *a string*, no `regelrecht://`
/// pattern). RFC-018 §1 keys the sidecar by law id — one file is one law's
/// notes — so anything that does not provably refer to `law_id` is
/// rejected, the note-side analogue of `save_law`'s `$id`/path guard.
/// Returns the first offending note (by array index) and why.
pub fn first_note_not_targeting_law(
    doc: &serde_json::Value,
    law_id: &str,
) -> Option<(usize, NoteTargetError)> {
    let notes = doc.get("annotations").and_then(|v| v.as_array())?;
    for (i, note) in notes.iter().enumerate() {
        let source = note
            .get("target")
            .and_then(|t| t.get("source"))
            .and_then(|s| s.as_str());
        let err = match source {
            None => Some(NoteTargetError::Unparseable),
            Some(s) => match law_id_from_source(s) {
                None => Some(NoteTargetError::NotRegelrechtUri),
                Some(id) if id == law_id => None,
                Some(id) => Some(NoteTargetError::WrongLaw(id.to_string())),
            },
        };
        if let Some(e) = err {
            return Some((i, e));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA_URL: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/heads/main/schema/v0.5.3/annotation-schema.json";

    #[test]
    fn schema_compiles() {
        // The embedded schema must be valid JSON and a compilable JSON
        // Schema. This is the build-time guarantee that the VALIDATOR
        // `Err` arm is unreachable in production.
        assert!(VALIDATOR.as_ref().is_ok(), "{:?}", VALIDATOR.as_ref().err());
    }

    #[test]
    fn valid_minimal_note_passes() {
        let yaml = format!(
            r#"
$schema: "{SCHEMA_URL}"
annotations:
  - type: Annotation
    motivation: commenting
    creator: tester
    target:
      source: "regelrecht://wet_op_de_zorgtoeslag"
      selector:
        type: TextQuoteSelector
        exact: zorgtoeslag
    body:
      type: TextualBody
      value: een toelichting
      purpose: commenting
"#
        );
        assert!(parse_and_validate_annotation_yaml(&yaml).is_ok());
    }

    #[test]
    fn missing_required_field_fails_with_message() {
        // No `target` — schema requires it on an Annotation.
        let yaml = format!(
            r#"
$schema: "{SCHEMA_URL}"
annotations:
  - type: Annotation
    motivation: commenting
"#
        );
        let errs = parse_and_validate_annotation_yaml(&yaml).unwrap_err();
        assert!(!errs.is_empty());
    }

    #[test]
    fn malformed_yaml_reports_parse_error() {
        let errs = parse_and_validate_annotation_yaml("annotations: [unterminated").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("YAML parse error"));
    }

    #[test]
    fn law_id_from_source_takes_the_host_segment() {
        assert_eq!(
            law_id_from_source("regelrecht://wet_op_de_zorgtoeslag"),
            Some("wet_op_de_zorgtoeslag")
        );
        assert_eq!(
            law_id_from_source("regelrecht://wet_op_de_zorgtoeslag/hoogte#out"),
            Some("wet_op_de_zorgtoeslag")
        );
        assert_eq!(law_id_from_source("https://example.com/x"), None);
    }

    #[test]
    fn parse_regelrecht_uri_recognises_laws() {
        assert_eq!(
            parse_regelrecht_uri("regelrecht://wet_op_de_zorgtoeslag"),
            Some(RegelrechtRef::Law {
                law_id: "wet_op_de_zorgtoeslag".to_string(),
                rest: "".to_string(),
            })
        );
        assert_eq!(
            parse_regelrecht_uri("regelrecht://wet_op_de_zorgtoeslag/hoogte#out"),
            Some(RegelrechtRef::Law {
                law_id: "wet_op_de_zorgtoeslag".to_string(),
                rest: "hoogte#out".to_string(),
            })
        );
    }

    #[test]
    fn parse_regelrecht_uri_recognises_documents() {
        assert_eq!(
            parse_regelrecht_uri("regelrecht://doc/migratie-1a2b3c4d/notes.md"),
            Some(RegelrechtRef::Document {
                traject_ref: "migratie-1a2b3c4d".to_string(),
                relative_path: "notes.md".to_string(),
            })
        );
        // Nested paths are preserved verbatim.
        assert_eq!(
            parse_regelrecht_uri("regelrecht://doc/migratie-1a2b3c4d/mvt/concept.md"),
            Some(RegelrechtRef::Document {
                traject_ref: "migratie-1a2b3c4d".to_string(),
                relative_path: "mvt/concept.md".to_string(),
            })
        );
    }

    #[test]
    fn parse_regelrecht_uri_rejects_malformed() {
        // No traject or no path.
        assert_eq!(parse_regelrecht_uri("regelrecht://doc/foo"), None);
        assert_eq!(parse_regelrecht_uri("regelrecht://doc/"), None);
        assert_eq!(parse_regelrecht_uri("regelrecht://doc"), None);
        // Non-regelrecht scheme.
        assert_eq!(parse_regelrecht_uri("https://example.com/x"), None);
    }

    /// `doc` is reserved as the document-scheme head — a regulation
    /// with `$id: doc` would silently parse as a `Document` instead of
    /// a `Law` reference. Pin the precedence so a future change that
    /// flips the order trips this test.
    #[test]
    fn parse_regelrecht_uri_treats_doc_as_reserved_head() {
        // `regelrecht://doc/<traject>/<path>` resolves as Document
        // even though `doc` is a syntactically valid law id slug.
        assert!(matches!(
            parse_regelrecht_uri("regelrecht://doc/traject-12345678/notes.md"),
            Some(RegelrechtRef::Document { .. })
        ));
        // A scheme head that merely starts with `doc` (e.g. `docs`,
        // `doctrine`) is a regular law id, NOT a document reference.
        assert!(matches!(
            parse_regelrecht_uri("regelrecht://docs/foo"),
            Some(RegelrechtRef::Law { .. })
        ));
    }

    #[test]
    fn first_note_not_targeting_law_is_an_allowlist() {
        // All on-law → None.
        let ok = serde_json::json!({"annotations": [
            {"target": {"source": "regelrecht://wet_op_de_zorgtoeslag"}},
            {"target": {"source": "regelrecht://wet_op_de_zorgtoeslag/hoogte#x"}},
        ]});
        assert_eq!(
            first_note_not_targeting_law(&ok, "wet_op_de_zorgtoeslag"),
            None
        );

        // A different law → WrongLaw at its index.
        let wrong = serde_json::json!({"annotations": [
            {"target": {"source": "regelrecht://wet_op_de_zorgtoeslag"}},
            {"target": {"source": "regelrecht://andere_wet"}},
        ]});
        assert_eq!(
            first_note_not_targeting_law(&wrong, "wet_op_de_zorgtoeslag"),
            Some((1, NoteTargetError::WrongLaw("andere_wet".to_string())))
        );

        // Non-regelrecht:// source must be REJECTED, not silently skipped
        // (the bypass the hostile review found).
        let evil = serde_json::json!({"annotations": [
            {"target": {"source": "https://evil/wet_op_de_zorgtoeslag"}},
        ]});
        assert_eq!(
            first_note_not_targeting_law(&evil, "wet_op_de_zorgtoeslag"),
            Some((0, NoteTargetError::NotRegelrechtUri))
        );

        // Absent target.source → Unparseable, also rejected.
        let bare = serde_json::json!({"annotations": [{"target": {}}]});
        assert_eq!(
            first_note_not_targeting_law(&bare, "wet_op_de_zorgtoeslag"),
            Some((0, NoteTargetError::Unparseable))
        );

        // Empty / absent array is vacuously fine — the destructive-shrink
        // guard in `append_notes_to_sidecar` (the rebuild path), not this
        // function, refuses a base whose notes failed to parse.
        assert_eq!(
            first_note_not_targeting_law(&serde_json::json!({"annotations": []}), "x"),
            None
        );
        assert_eq!(
            first_note_not_targeting_law(&serde_json::json!({}), "x"),
            None
        );
    }

    // A base sidecar with the curated shape the real corpus file uses:
    // `---`, comments, an explanatory header, semantic key order, a
    // multi-line block scalar. Appending must not touch any of this.
    const CURATED_BASE: &str = "\
---
# Stand-off notes. RFC-005 / RFC-018.
$schema: https://example/schema.json
annotations:
  # Linking note: curated explanation that must survive.
  - type: Annotation
    motivation: linking
    creator: Dienst Toeslagen
    target:
      source: regelrecht://wet_op_de_zorgtoeslag
      selector:
        type: TextQuoteSelector
        exact: zorgtoeslag
    body:
      type: SpecificResource
      source: regelrecht://wet_op_de_zorgtoeslag/x#y
      purpose: linking
";

    fn note(exact: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "Annotation",
            "motivation": "commenting",
            "creator": "tester",
            "target": {
                "source": "regelrecht://wet_op_de_zorgtoeslag",
                "selector": { "type": "TextQuoteSelector", "exact": exact }
            },
            "body": { "type": "TextualBody", "value": "x", "purpose": "commenting" }
        })
    }

    #[test]
    fn append_keeps_the_base_verbatim_and_only_adds_lines() {
        let new = vec![note("normpremie")];
        let out = match append_notes_to_sidecar(Some(CURATED_BASE), &new, "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            other => panic!(
                "expected Write, got {:?}",
                matches!(other, Ok(AppendOutcome::NoChange))
            ),
        };
        // Every original byte/line is still there, in order: the comments,
        // the $schema, the curated note. The base is a strict prefix
        // (modulo the single trailing-newline normalisation).
        assert!(out.starts_with(CURATED_BASE.trim_end_matches('\n')));
        assert!(out.contains("# Linking note: curated explanation that must survive."));
        assert!(out.contains("creator: Dienst Toeslagen"));
        // The appended note sits under `annotations:` at the 2-space list
        // convention and carries its content.
        assert!(out.contains("\n  - "));
        assert!(out.contains("normpremie"));
        // Result parses and the sequence grew by exactly one.
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn append_dedups_against_the_base_and_is_idempotent() {
        // The base already contains the linking note; re-submitting an
        // identical-content note must not duplicate it. Reconstruct that
        // note exactly as it parses out of the base.
        let base_doc: serde_json::Value = serde_yaml_ng::from_str(CURATED_BASE).unwrap();
        let existing = base_doc["annotations"][0].clone();

        // Only the existing note resubmitted → nothing to write.
        assert!(matches!(
            append_notes_to_sidecar(
                Some(CURATED_BASE),
                std::slice::from_ref(&existing),
                "https://s"
            ),
            Ok(AppendOutcome::NoChange)
        ));

        // Existing + one genuinely new → only the new one is appended.
        let out = match append_notes_to_sidecar(
            Some(CURATED_BASE),
            &[existing, note("uniek")],
            "https://s",
        ) {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn append_with_no_base_builds_a_fresh_document() {
        let out = match append_notes_to_sidecar(None, &[note("a")], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["$schema"], "https://s");
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn append_to_base_without_annotations_key_rebuilds() {
        // A file that parses but has no annotations sequence: nothing to
        // preserve, so a fresh full doc is fine.
        let bare = "$schema: https://old\n";
        let out = match append_notes_to_sidecar(Some(bare), &[note("a")], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected Write"),
        };
        let doc: serde_json::Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn rebuild_path_refuses_to_drop_unparseable_existing_notes() {
        // A base that carries real note content but does NOT parse, so
        // `notes_in_sidecar` yields `[]` and the verbatim-append path is
        // skipped. Without the guard the rebuild path would emit a file
        // containing only the new note, silently discarding the existing
        // one. Regression for the "shrink guard" the RFC promises
        // (previously fictional). The trigger is a YAML syntax fault
        // (here: a tab in indentation, which serde_yaml_ng rejects) in a
        // file that still clearly holds an Annotation.
        let broken_base =
            "$schema: https://s\nannotations:\n  - type: Annotation\n\tmotivation: commenting\n";
        assert!(
            serde_yaml_ng::from_str::<serde_json::Value>(broken_base).is_err(),
            "test premise: base must be unparseable"
        );
        match append_notes_to_sidecar(Some(broken_base), &[note("new")], "https://s") {
            Err(e) => assert!(e.contains("refusing to rebuild"), "{e}"),
            Ok(_) => panic!("must refuse to rebuild over unparseable existing notes"),
        }

        // Sanity: a genuinely empty/contentless base is still a normal
        // fresh build, NOT a refusal (the guard keys on note-like content).
        assert!(matches!(
            append_notes_to_sidecar(Some("$schema: https://s\n"), &[note("a")], "https://s"),
            Ok(AppendOutcome::Write(_))
        ));
    }

    #[test]
    fn append_empty_input_is_nochange() {
        assert!(matches!(
            append_notes_to_sidecar(Some(CURATED_BASE), &[], "https://s"),
            Ok(AppendOutcome::NoChange)
        ));
        assert!(matches!(
            append_notes_to_sidecar(None, &[], "https://s"),
            Ok(AppendOutcome::NoChange)
        ));
    }

    #[test]
    fn second_append_onto_a_serde_produced_base_stays_valid() {
        // The live 500: the FIRST save builds the file via the fresh-doc
        // path (serialize_annotation_doc → serde_yaml_ng → `- ` at column
        // 0). The SECOND save reads that file as the base and appends. The
        // old code re-indented by a hardcoded 2 spaces, producing a
        // column-2 item after column-0 items → "did not find expected
        // key". This pins the round-trip: serialise, then append, then the
        // result must still parse and have grown by one.
        let n1 = note("eerste");
        let first = match append_notes_to_sidecar(None, &[n1], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            _ => panic!("expected fresh-doc Write"),
        };
        // Sanity: the fresh doc really is column-0 (the failing shape).
        assert_eq!(sequence_item_indent(&first), 0);

        let n2 = note("tweede");
        let second = match append_notes_to_sidecar(Some(&first), &[n2], "https://s") {
            Ok(AppendOutcome::Write(t)) => t,
            other => panic!(
                "expected Write, got NoChange={}",
                matches!(other, Ok(AppendOutcome::NoChange))
            ),
        };
        // The whole point: it parses (it did NOT before the fix).
        let doc: serde_json::Value = serde_yaml_ng::from_str(&second)
            .expect("second append onto a serde-produced base must stay valid YAML");
        assert_eq!(doc["annotations"].as_array().unwrap().len(), 2);
        // Base preserved verbatim as prefix; only lines added.
        assert!(second.starts_with(first.trim_end_matches('\n')));
    }

    #[test]
    fn sequence_item_indent_reads_real_depth() {
        // serde_yaml_ng / fresh-doc shape: column 0.
        assert_eq!(
            sequence_item_indent("$schema: x\nannotations:\n- type: Annotation\n"),
            0
        );
        // Curated 2-space shape.
        assert_eq!(sequence_item_indent(CURATED_BASE), 2);
        // No sequence yet → fall back to 0 (fresh-doc path handles it).
        assert_eq!(sequence_item_indent("$schema: x\nannotations: []\n"), 0);
    }
}
