//! Lift a law file to schema v0.7.1, and leave a newer one as it is.
//!
//! A file that already declares a newer version (v0.8.0 and up) is left
//! alone: a newer version may carry fields v0.7.1 lacks, such as
//! `produces.submission` (v0.8.0) or `produces` as a list (v0.8.1), so
//! moving its `$schema` back would claim a contract the file breaks. It
//! comes back unchanged and is validated
//! against its own version.
//!
//! v0.7.1 is v0.7.0 with six field descriptions corrected and nothing else, so
//! every statement below about v0.7.0's shape holds for the target unchanged.
//! The target is v0.7.1 rather than v0.7.0 itself because files stamped
//! v0.7.1 exist: migrating one of those must not move its `$schema` back a
//! version. The harvester now stamps a newer version, additive on v0.7.1, so
//! such a file passes through unchanged (see above).
//!
//! Two jobs, and they are deliberately unequal in ambition.
//!
//! A **bare harvested law** carries no model at all, so the only thing that
//! stands between it and v0.7.0 is the `$schema` line. Measured rather than
//! assumed: between v0.3.1 and v0.7.0 nothing a harvest writes was removed or
//! tightened — the top-level and per-article shapes only gained fields
//! (`procedure`, `valid_to`, `waterschap_code`, `placement`) and the
//! `regulatory_layer` enum only grew. So the bump is a one-line rewrite, and
//! this module leaves the remaining bytes untouched: a harvested file keeps
//! its folded block scalars instead of being reformatted by a round-trip
//! through the YAML serializer.
//!
//! An **enriched law** written against the pre-consolidation v0.7.0 carries
//! `untranslatables`, `norm_gaps` and `enables`, which that version no longer
//! has. Those are converted, and the conversion is deliberately narrow: it
//! moves what has a counterpart and it *refuses* to invent what does not.
//! Where a required field of the new shape has no source in the old one, the
//! field is left out and a [`Blocker`] is recorded. The result is then a file
//! that fails schema validation on exactly the places a human still has to
//! decide, which is the point — a conversion that fills those gaps with a
//! plausible default is indistinguishable from a correct one.
//!
//! What the old fields map to:
//!
//! | old (`untranslatables`)      | new (`markings`)        |
//! |------------------------------|-------------------------|
//! | `construct`                  | `about`                 |
//! | `kind: engine_operation`     | `resolution: operation` |
//! | `kind: model_form`           | `resolution: model`     |
//! | `reason`                     | `reason`                |
//! | `suggestion`                 | `resolved_by`           |
//! | `blocks`                     | `target`                |
//! | `legal_text_excerpt`         | `legal_text_excerpt`    |
//! | `accepted`                   | `accepted`              |
//!
//! | old (`norm_gaps`)            | new (`open_terms`)   |
//! |------------------------------|----------------------|
//! | `norm`                       | `description`        |
//! | `expected_source`            | `expected_source`    |
//! | `decided_per_case_by`        | `decided_per_case_by`|
//! | `kind`, `blocks`, `searched`, `legal_text_excerpt` | — (dropped) |
//!
//! `enables` is dropped whole: the contract that consolidated the four flag
//! fields into two removed it, and there is nothing in v0.7.0 that holds it.

use regelrecht_shared::parse_schema_version;
use serde_yaml_ng::{Mapping, Value};

/// `$schema` URL a migrated file declares.
pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.7.1/schema/v0.7.1/schema.json";

/// The schema version this module migrates to.
pub const TARGET_VERSION: &str = "v0.7.1";

/// A place where the conversion could not proceed without inventing a value.
///
/// Deliberately not an error that aborts the run: the whole point is to get
/// the complete list in one pass, so the file is written with the field left
/// out and the blocker recorded beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocker {
    /// Article `number` the item sits in.
    pub article: String,
    /// Which converted list the item came from, e.g. `norm_gaps`.
    pub source_field: &'static str,
    /// Index of the item within that list, zero-based.
    pub index: usize,
    /// The field of the new shape that could not be filled.
    pub missing: &'static str,
    /// Why it could not be filled.
    pub reason: String,
}

/// Information the new shape has no field for, dropped on purpose.
///
/// Separate from [`Blocker`] because it does not stop the file from
/// validating: nothing is missing, something is gone. Reported so the loss is
/// visible instead of silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub article: String,
    pub field: &'static str,
    pub count: usize,
}

/// Result of a migration run.
#[derive(Debug, Clone)]
pub struct Migration {
    /// The migrated YAML.
    pub yaml: String,
    /// The version the file declared before migration, when recognised.
    pub from_version: Option<String>,
    /// The version the result declares: [`TARGET_VERSION`], or the file's own
    /// version when that is newer and the file was left alone.
    pub to_version: String,
    /// Whether anything beyond the `$schema` line changed.
    pub structural_changes: bool,
    /// Required fields of the new shape with no source in the old one.
    pub blockers: Vec<Blocker>,
    /// Old fields with no counterpart in v0.7.0.
    pub dropped: Vec<Dropped>,
    /// Schema errors of the *migrated* file, against `to_version`.
    pub schema_errors: Vec<String>,
}

impl Migration {
    /// True when the migrated file validates and nothing had to be left out.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.blockers.is_empty() && self.schema_errors.is_empty()
    }
}

/// Migrate one law file to [`TARGET_VERSION`], or leave it alone when it
/// declares a newer version.
///
/// `Err` is reserved for a file this module cannot read at all — unparseable
/// YAML, or a document that is not a mapping. Everything else, including a
/// migration that leaves required fields unfilled, comes back as an `Ok`
/// carrying the evidence.
pub fn migrate(yaml: &str) -> Result<Migration, String> {
    let mut doc: Value =
        serde_yaml_ng::from_str(yaml).map_err(|e| format!("cannot parse YAML: {e}"))?;

    let from_version = as_json(&doc)
        .as_ref()
        .and_then(regelrecht_engine::schema::detect_version)
        .map(str::to_string);

    if let Some(newer) = from_version.as_deref().filter(|v| is_newer_than_target(v)) {
        let schema_errors = as_json(&doc).map_or_else(
            || vec!["file is not representable as JSON".to_string()],
            |json| {
                regelrecht_engine::schema::validation_errors_for(newer, &json)
                    .unwrap_or_else(|e| vec![format!("schema validation failed: {e}")])
            },
        );
        return Ok(Migration {
            yaml: yaml.to_string(),
            to_version: newer.to_string(),
            from_version,
            structural_changes: false,
            blockers: Vec::new(),
            dropped: Vec::new(),
            schema_errors,
        });
    }

    let mut blockers = Vec::new();
    let mut dropped = Vec::new();

    let root = doc
        .as_mapping_mut()
        .ok_or_else(|| "top level of the document is not a mapping".to_string())?;

    let mut structural_changes = false;
    if let Some(articles) = root.get_mut("articles").and_then(Value::as_sequence_mut) {
        for article in articles.iter_mut() {
            structural_changes |= migrate_article(article, &mut blockers, &mut dropped);
        }
    }

    // Only rewrite the whole file when something below the header moved. A
    // bare harvested law keeps its exact bytes apart from one line, which
    // matters: a round-trip through the serializer reflows every folded
    // block scalar in the statutory text and buries the real change.
    let migrated = if structural_changes {
        root.insert(
            Value::String("$schema".into()),
            Value::String(SCHEMA_URL.into()),
        );
        let body = serde_yaml_ng::to_string(&doc).map_err(|e| format!("cannot serialize: {e}"))?;
        format!("---\n{body}")
    } else {
        rewrite_schema_line(yaml)?
    };

    let schema_errors = match serde_yaml_ng::from_str::<Value>(&migrated)
        .ok()
        .and_then(|v| as_json(&v))
    {
        Some(json) => regelrecht_engine::schema::validation_errors_for(TARGET_VERSION, &json)
            .unwrap_or_else(|e| vec![format!("schema validation failed: {e}")]),
        None => vec!["migrated file is not valid YAML".to_string()],
    };

    Ok(Migration {
        yaml: migrated,
        to_version: TARGET_VERSION.to_string(),
        from_version,
        structural_changes,
        blockers,
        dropped,
        schema_errors,
    })
}

/// Whether `version` (`vX.Y.Z`) is newer than [`TARGET_VERSION`]. An
/// unparseable version is not newer, so it is migrated as before.
fn is_newer_than_target(version: &str) -> bool {
    match (
        parse_schema_version(version),
        parse_schema_version(TARGET_VERSION),
    ) {
        (Some(v), Some(t)) => v > t,
        // Unparseable: not newer, so the file is migrated as before.
        _ => false,
    }
}

fn as_json(value: &Value) -> Option<serde_json::Value> {
    serde_json::to_value(value).ok()
}

/// Replace the `$schema` value in place, leaving every other byte alone.
///
/// Two shapes occur in practice and both are handled: the inline form
/// (`$schema: <url>` on one line), which is what the harvester writes, and
/// the block form (`$schema: >-` and friends), whose value sits on the
/// indented lines below the marker. The block form is rewritten to the
/// inline form, marker and continuation together — replacing only the marker
/// line would leave the old URL behind as a second line of the same scalar,
/// and YAML would fold the two into one string carrying both URLs.
///
/// Anything else — no declaration at all, or a value this function cannot
/// delimit — falls through to an error rather than a silent no-op, because a
/// bump that did not happen is the failure mode this whole module exists to
/// prevent.
fn rewrite_schema_line(yaml: &str) -> Result<String, String> {
    let mut out = String::with_capacity(yaml.len() + SCHEMA_URL.len());
    let mut replaced = false;
    let mut lines = yaml.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let body = line.trim_end_matches(['\n', '\r']);
        if replaced || !body.starts_with("$schema:") {
            out.push_str(line);
            continue;
        }

        let rest = body["$schema:".len()..].trim();
        // Any block scalar, however it is spelled. YAML allows a chomping
        // indicator and an explicit indentation indicator after `>` or `|`
        // (`>-`, `|2`, `>-2`), so enumerating the spellings misses forms that
        // then fall through to the inline branch, which rewrites the marker
        // line and leaves the continuation standing: the corruption this
        // function exists to prevent, reached through a rarer door.
        let is_block = rest.starts_with(['>', '|']);
        if !is_block && rest.is_empty() {
            return Err(
                "`$schema:` carries no value and no block scalar marker to rewrite".to_string(),
            );
        }
        if !is_block && rest.starts_with(['&', '*', '!']) {
            return Err(format!(
                "`$schema:` is written as `{rest}`, which is neither an inline value nor a block scalar"
            ));
        }

        let newline = &line[body.len()..];
        out.push_str("$schema: ");
        out.push_str(SCHEMA_URL);
        out.push_str(newline);
        replaced = true;

        if is_block {
            // Drop the scalar's continuation. A block scalar at the top level
            // runs until the first line that is neither indented nor empty,
            // so an empty line only ends it when nothing indented follows.
            let mut pending_blanks = String::new();
            while let Some(next) = lines.peek() {
                let next_body = next.trim_end_matches(['\n', '\r']);
                if next_body.trim().is_empty() {
                    pending_blanks.push_str(next);
                    lines.next();
                } else if next_body.starts_with([' ', '\t']) {
                    pending_blanks.clear();
                    lines.next();
                } else {
                    break;
                }
            }
            // Blank lines that turned out to sit after the scalar rather than
            // inside it are not ours to remove.
            out.push_str(&pending_blanks);
        }
    }
    if replaced {
        Ok(out)
    } else {
        Err("no `$schema:` line to rewrite".to_string())
    }
}

/// Convert one article's `machine_readable` section. Returns true when
/// anything changed.
fn migrate_article(
    article: &mut Value,
    blockers: &mut Vec<Blocker>,
    dropped: &mut Vec<Dropped>,
) -> bool {
    let number = article
        .get("number")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();

    let Some(mr) = article
        .get_mut("machine_readable")
        .and_then(Value::as_mapping_mut)
    else {
        return false;
    };

    let mut changed = false;

    if let Some(old) = mr.remove("untranslatables") {
        changed = true;
        let items = old.as_sequence().cloned().unwrap_or_default();
        let mut markings = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            markings.push(Value::Mapping(to_marking(item, &number, index, blockers)));
        }
        if !markings.is_empty() {
            mr.insert(Value::String("markings".into()), Value::Sequence(markings));
        }
    }

    if let Some(old) = mr.remove("norm_gaps") {
        changed = true;
        let items = old.as_sequence().cloned().unwrap_or_default();
        let mut terms = Vec::with_capacity(items.len());
        let mut losses: Vec<(&'static str, usize)> = vec![
            ("norm_gaps.kind", 0),
            ("norm_gaps.blocks", 0),
            ("norm_gaps.searched", 0),
            ("norm_gaps.legal_text_excerpt", 0),
        ];
        for (index, item) in items.iter().enumerate() {
            let term = to_open_term(item, &number, index, blockers, &mut losses);
            terms.push(Value::Mapping(term));
        }
        for (field, count) in losses {
            if count > 0 {
                dropped.push(Dropped {
                    article: number.clone(),
                    field,
                    count,
                });
            }
        }
        if !terms.is_empty() {
            // Merge rather than overwrite: an article may already carry
            // open_terms of its own, and a norm gap that becomes one belongs
            // beside them, not instead of them.
            let existing = mr
                .get("open_terms")
                .and_then(Value::as_sequence)
                .cloned()
                .unwrap_or_default();
            let merged = existing.into_iter().chain(terms).collect();
            mr.insert(Value::String("open_terms".into()), Value::Sequence(merged));
        }
    }

    // Fields v0.7.0 removed outright. `structural_choices` was never written
    // by anything, only read; `enables` and `legal_basis_for` were.
    for field in ["enables", "legal_basis_for", "structural_choices"] {
        if let Some(old) = mr.remove(field) {
            changed = true;
            let count = old.as_sequence().map_or(1, Vec::len);
            dropped.push(Dropped {
                article: number.clone(),
                field: match field {
                    "enables" => "enables",
                    "legal_basis_for" => "legal_basis_for",
                    _ => "structural_choices",
                },
                count,
            });
        }
    }

    changed
}

/// Build a `markings` item.
fn to_marking(item: &Value, article: &str, index: usize, blockers: &mut Vec<Blocker>) -> Mapping {
    let mut out = Mapping::new();
    let block = |missing: &'static str, reason: &str, blockers: &mut Vec<Blocker>| {
        blockers.push(Blocker {
            article: article.to_string(),
            source_field: "untranslatables",
            index,
            missing,
            reason: reason.to_string(),
        });
    };

    match item.get("construct") {
        Some(v @ Value::String(_)) => {
            out.insert(Value::String("about".into()), v.clone());
        }
        _ => block("about", "no `construct` to carry over", blockers),
    }

    match item.get("kind").and_then(Value::as_str) {
        Some("engine_operation") => {
            out.insert(
                Value::String("resolution".into()),
                Value::String("operation".into()),
            );
        }
        Some("model_form") => {
            out.insert(
                Value::String("resolution".into()),
                Value::String("model".into()),
            );
        }
        Some(other) => block(
            "resolution",
            &format!("`kind: {other}` is not one of engine_operation, model_form"),
            blockers,
        ),
        None => block(
            "resolution",
            "no `kind`, and nothing else in the item says whether the engine or the model has to change",
            blockers,
        ),
    }

    // The two prose fields carry over one to one, and neither stands in for
    // the other. `reason` is the diagnosis and `suggestion` the wanted change;
    // the change follows from the diagnosis and the diagnosis is not
    // recoverable from the change, so a missing one is a blocker rather than a
    // copy of its neighbour.
    match item.get("reason") {
        Some(v @ Value::String(_)) => {
            out.insert(Value::String("reason".into()), v.clone());
        }
        _ => block(
            "reason",
            "no `reason`, and `suggestion` says what would fix it rather than why it does not fit",
            blockers,
        ),
    }

    match item.get("suggestion") {
        Some(v @ Value::String(_)) => {
            out.insert(Value::String("resolved_by".into()), v.clone());
        }
        _ => block(
            "resolved_by",
            "no `suggestion`, and `reason` says why it does not fit rather than what would fix it",
            blockers,
        ),
    }

    match item.get("blocks") {
        Some(v @ Value::Sequence(_)) => {
            out.insert(Value::String("target".into()), v.clone());
        }
        _ => block(
            "target",
            "no `blocks`; an empty list is a claim that the article stays executable and cannot be assumed",
            blockers,
        ),
    }

    match item.get("legal_text_excerpt") {
        Some(v @ Value::String(_)) => {
            out.insert(Value::String("legal_text_excerpt".into()), v.clone());
        }
        _ => block("legal_text_excerpt", "no `legal_text_excerpt`", blockers),
    }

    if let Some(v @ Value::Bool(_)) = item.get("accepted") {
        out.insert(Value::String("accepted".into()), v.clone());
    }

    out
}

/// Build an `open_terms` item from a norm gap.
fn to_open_term(
    item: &Value,
    article: &str,
    index: usize,
    blockers: &mut Vec<Blocker>,
    losses: &mut [(&'static str, usize)],
) -> Mapping {
    let mut out = Mapping::new();

    // `id` and `type` are both required and neither has a counterpart in a
    // norm gap. Slugging `norm` into an id would be a fixed rule, but the id
    // is what `implements` binds to, so a synthetic one is a name the corpus
    // would then have to keep. Left to a human, like the type.
    blockers.push(Blocker {
        article: article.to_string(),
        source_field: "norm_gaps",
        index,
        missing: "id",
        reason: "a norm gap carries no identifier, and `implements` binds to this name".to_string(),
    });

    if let Some(v @ Value::String(_)) = item.get("norm") {
        out.insert(Value::String("description".into()), v.clone());
    }

    blockers.push(Blocker {
        article: article.to_string(),
        source_field: "norm_gaps",
        index,
        missing: "type",
        reason: "a norm gap names the open norm, not the data type of the value that fills it"
            .to_string(),
    });

    if let Some(v @ Value::String(_)) = item.get("expected_source") {
        out.insert(Value::String("expected_source".into()), v.clone());
    }
    if let Some(v @ Value::String(_)) = item.get("decided_per_case_by") {
        out.insert(Value::String("decided_per_case_by".into()), v.clone());
    }

    for (field, count) in losses.iter_mut() {
        let key = field.strip_prefix("norm_gaps.").unwrap_or(field);
        if item.get(key).is_some() {
            *count += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A harvested law as the harvester writes one: no model, and statutory
    /// text in a folded block scalar, which is what the byte-preservation
    /// test below is about.
    const BARE: &str = r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.3.1/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://example.org/law
articles:
  - number: '1'
    text: >-
      De eerste zin.


      De tweede zin.
    url: https://example.org/law#Artikel1
"#;

    #[test]
    fn a_bare_v0_3_1_law_reaches_v0_7_0_and_validates() {
        let m = migrate(BARE).expect("migrates");
        assert_eq!(m.from_version.as_deref(), Some("v0.3.1"));
        assert!(m.yaml.contains(SCHEMA_URL));
        assert!(
            m.schema_errors.is_empty(),
            "expected a valid v0.7.0 file, got {:?}",
            m.schema_errors
        );
        assert!(m.is_clean());
    }

    #[test]
    fn a_bare_law_keeps_every_byte_but_the_schema_line() {
        let m = migrate(BARE).expect("migrates");
        assert!(!m.structural_changes);
        let before: Vec<&str> = BARE.lines().skip(2).collect();
        let after: Vec<&str> = m.yaml.lines().skip(2).collect();
        assert_eq!(
            before, after,
            "the folded block scalar must survive the bump untouched"
        );
    }

    #[test]
    fn a_v0_8_0_law_is_not_moved_back() {
        let v080 = BARE.replace(
            "refs/heads/main/schema/v0.3.1",
            "refs/tags/schema-v0.8.0/schema/v0.8.0",
        );
        let m = migrate(&v080).expect("migrates");
        assert_eq!(m.from_version.as_deref(), Some("v0.8.0"));
        assert_eq!(m.to_version, "v0.8.0");
        assert_eq!(m.yaml, v080, "a newer file must come back byte for byte");
        assert!(m.is_clean(), "{:?}", m.schema_errors);
    }

    #[test]
    fn migrating_twice_is_the_same_as_migrating_once() {
        let once = migrate(BARE).expect("migrates");
        let twice = migrate(&once.yaml).expect("migrates");
        assert_eq!(once.yaml, twice.yaml);
    }

    #[test]
    fn a_file_without_an_inline_schema_line_is_refused() {
        let err = migrate("articles: []\n").expect_err("no $schema to rewrite");
        assert!(err.contains("$schema"), "got {err}");
    }

    /// The shape 94 corpus files are in: the URL sits on the indented line
    /// below a folded-block marker instead of on the key's own line.
    const BARE_FOLDED: &str = r#"---
$schema: >-
  https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.8/schema/v0.5.8/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://example.org/law
articles:
  - number: '1'
    text: >-
      De eerste zin.


      De tweede zin.
    url: https://example.org/law#Artikel1
"#;

    #[test]
    fn a_folded_schema_scalar_becomes_one_inline_line() {
        let m = migrate(BARE_FOLDED).expect("migrates");
        assert_eq!(m.from_version.as_deref(), Some("v0.5.8"));

        let schema_lines: Vec<&str> = m
            .yaml
            .lines()
            .filter(|l| l.starts_with("$schema"))
            .collect();
        assert_eq!(
            schema_lines,
            vec![format!("$schema: {SCHEMA_URL}").as_str()],
            "expected exactly one inline $schema line"
        );
        assert!(
            !m.yaml.contains("v0.5.8"),
            "the old URL must be gone, not folded in beside the new one"
        );
    }

    #[test]
    fn a_folded_schema_rewrite_touches_nothing_else() {
        let m = migrate(BARE_FOLDED).expect("migrates");
        assert!(!m.structural_changes);
        // Skip `---`, the marker line and its continuation.
        let before: Vec<&str> = BARE_FOLDED.lines().skip(3).collect();
        let after: Vec<&str> = m.yaml.lines().skip(2).collect();
        assert_eq!(
            before, after,
            "the folded block scalar of the statutory text must survive untouched"
        );
    }

    #[test]
    fn a_migrated_folded_law_validates_and_reports_the_new_version() {
        let m = migrate(BARE_FOLDED).expect("migrates");
        assert!(
            m.schema_errors.is_empty(),
            "expected a valid v0.7.0 file, got {:?}",
            m.schema_errors
        );
        assert!(m.is_clean());

        let doc: Value = serde_yaml_ng::from_str(&m.yaml).expect("migrated file parses");
        let json = as_json(&doc).expect("converts to JSON");
        assert_eq!(
            regelrecht_engine::schema::detect_version(&json),
            Some(TARGET_VERSION),
            "a second run must see the target version, not the old one"
        );
    }

    #[test]
    fn migrating_a_folded_law_twice_is_the_same_as_migrating_once() {
        let once = migrate(BARE_FOLDED).expect("migrates");
        let twice = migrate(&once.yaml).expect("migrates");
        assert_eq!(once.yaml, twice.yaml);
    }

    #[test]
    fn a_literal_block_schema_scalar_is_rewritten_too() {
        let yaml = BARE_FOLDED.replace("$schema: >-", "$schema: |-");
        let m = migrate(&yaml).expect("migrates");
        assert!(m.yaml.contains(&format!("$schema: {SCHEMA_URL}")));
        assert!(!m.yaml.contains("v0.5.8"));
        assert!(m.schema_errors.is_empty(), "{:?}", m.schema_errors);
    }

    /// A block scalar may carry an explicit indentation indicator on top of
    /// its chomping indicator (`>2`, `|-2`). Enumerating the six common
    /// spellings let those through to the inline branch, which rewrites the
    /// marker line and leaves the indented continuation standing: YAML then
    /// folds the new URL and the old one into a single scalar, the schema
    /// gate sees a document without a `$schema` it validates, and a later run
    /// reads the old version back out of the concatenation.
    #[test]
    fn a_block_scalar_with_an_indentation_indicator_is_rewritten_too() {
        for marker in [">2", ">-2", "|2", "|-2", ">+2"] {
            let yaml = BARE_FOLDED.replace("$schema: >-", &format!("$schema: {marker}"));
            let m = migrate(&yaml).expect("migrates");
            assert!(
                m.yaml.contains(&format!("$schema: {SCHEMA_URL}")),
                "{marker}: new URL missing"
            );
            assert!(
                !m.yaml.contains("v0.5.8"),
                "{marker}: old URL survived, the scalar is now two URLs: {}",
                m.yaml
            );
            assert!(
                m.schema_errors.is_empty(),
                "{marker}: {:?}",
                m.schema_errors
            );
            let reread: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(&m.yaml).expect("migrated file parses");
            let json = as_json(&reread).expect("as json");
            assert_eq!(
                regelrecht_engine::schema::detect_version(&json),
                Some(TARGET_VERSION),
                "{marker}: a second run must read the new version"
            );
        }
    }

    #[test]
    fn a_schema_key_that_is_neither_inline_nor_folded_is_refused() {
        let yaml = BARE_FOLDED.replace(
            "$schema: >-\n  https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.8/schema/v0.5.8/schema.json\n",
            "$schema:\n",
        );
        let err = migrate(&yaml).expect_err("an empty $schema is not rewritable");
        assert!(err.contains("$schema"), "got {err}");
    }

    /// Wraps `machine_readable` (already indented six spaces) in a law that
    /// is otherwise complete, so the only schema errors a test sees are the
    /// ones it is about.
    fn enriched(machine_readable: &str) -> String {
        format!(
            r#"---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000001
url: https://example.org/law
articles:
  - number: '1'
    text: Een zin.
    url: https://example.org/law#Artikel1
    machine_readable:
{machine_readable}"#
        )
    }

    #[test]
    fn a_complete_untranslatable_becomes_a_marking_without_a_blocker() {
        let yaml = enriched(
            r#"      untranslatables:
        - construct: afronden op hele euro's
          kind: engine_operation
          reason: de motor kent geen wettelijke afronding
          suggestion: een ROUND_HALF_UP-bewerking
          blocks: []
          legal_text_excerpt: naar boven afgerond op hele euro's
          accepted: false
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        assert!(m.structural_changes);
        assert_eq!(m.blockers, vec![]);
        assert!(
            m.schema_errors.is_empty(),
            "expected a valid file, got {:?}",
            m.schema_errors
        );
        assert!(m.yaml.contains("resolution: operation"));
        assert!(m.yaml.contains("about: afronden op hele euro's"));
        assert!(m.yaml.contains("resolved_by: een ROUND_HALF_UP-bewerking"));
        assert!(!m.yaml.contains("untranslatables"));
        // The diagnosis has a field of its own in the new shape and carries
        // over verbatim, beside the change it yields.
        assert!(
            m.yaml
                .contains("reason: de motor kent geen wettelijke afronding")
        );
        assert_eq!(m.dropped, vec![]);
    }

    #[test]
    fn an_untranslatable_without_a_reason_blocks_on_reason() {
        // Nothing else in the item says why the construct does not fit, and a
        // suggestion is not that: it is the change the reading yields, and the
        // reading cannot be read back out of it. Filling this from the
        // neighbour would make an examined gap and an unexamined one look
        // alike, which is what the field exists against.
        let yaml = enriched(
            r#"      untranslatables:
        - construct: kwantificeren over personen
          kind: model_form
          suggestion: een vorm voor een regel over een verzameling
          blocks: []
          legal_text_excerpt: een van de medebewoners
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        let missing: Vec<&str> = m.blockers.iter().map(|b| b.missing).collect();
        assert_eq!(missing, vec!["reason"]);
        assert!(!m.yaml.contains("reason:"));
        assert!(
            !m.schema_errors.is_empty(),
            "a marking without a reason must fail v0.7.0"
        );
    }

    #[test]
    fn model_form_becomes_resolution_model() {
        let yaml = enriched(
            r#"      untranslatables:
        - construct: kwantificeren over personen
          kind: model_form
          reason: het model kent alleen regels over een waarde
          suggestion: een vorm voor een regel over een verzameling
          blocks: []
          legal_text_excerpt: een van de medebewoners
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        assert!(m.yaml.contains("resolution: model"));
        assert_eq!(m.blockers, vec![]);
    }

    #[test]
    fn an_untranslatable_without_blocks_blocks_on_target() {
        let yaml = enriched(
            r#"      untranslatables:
        - construct: kwantificeren over personen
          kind: model_form
          reason: het model kent alleen regels over een waarde
          suggestion: een vorm voor een regel over een verzameling
          legal_text_excerpt: een van de medebewoners
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        let missing: Vec<&str> = m.blockers.iter().map(|b| b.missing).collect();
        assert_eq!(missing, vec!["target"]);
        // The claim is left out rather than guessed at, so the file says so.
        assert!(
            !m.schema_errors.is_empty(),
            "a marking without target must fail v0.7.0"
        );
        assert!(!m.is_clean());
    }

    #[test]
    fn an_untranslatable_without_a_suggestion_blocks_on_resolved_by() {
        let yaml = enriched(
            r#"      untranslatables:
        - construct: kwantificeren over personen
          kind: model_form
          reason: het model kent geen vorm hiervoor
          blocks: []
          legal_text_excerpt: een van de medebewoners
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        let missing: Vec<&str> = m.blockers.iter().map(|b| b.missing).collect();
        assert_eq!(missing, vec!["resolved_by"]);
        assert!(!m.yaml.contains("resolved_by"));
    }

    #[test]
    fn a_norm_gap_becomes_an_open_term_that_is_two_fields_short() {
        let yaml = enriched(
            r#"      norm_gaps:
        - norm: de standaardpremie
          kind: delegated
          blocks: []
          expected_source: Regeling zorgverzekering
          searched: geen regeling gevonden
          legal_text_excerpt: bij ministeriele regeling
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        assert!(m.yaml.contains("description: de standaardpremie"));
        assert!(m.yaml.contains("expected_source: Regeling zorgverzekering"));
        let missing: Vec<&str> = m.blockers.iter().map(|b| b.missing).collect();
        assert_eq!(missing, vec!["id", "type"]);
        assert!(
            !m.schema_errors.is_empty(),
            "an open term without id and type must fail v0.7.0"
        );
        let gone: Vec<&str> = m.dropped.iter().map(|d| d.field).collect();
        assert_eq!(
            gone,
            vec![
                "norm_gaps.kind",
                "norm_gaps.blocks",
                "norm_gaps.searched",
                "norm_gaps.legal_text_excerpt"
            ]
        );
    }

    #[test]
    fn converted_norm_gaps_join_the_open_terms_already_there() {
        let yaml = enriched(
            r#"      open_terms:
        - id: standaardpremie
          type: amount
      norm_gaps:
        - norm: redelijkerwijs
          kind: open
          blocks: []
          decided_per_case_by: de Dienst Toeslagen
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        let doc: Value = serde_yaml_ng::from_str(&m.yaml).expect("valid yaml");
        let terms = doc["articles"][0]["machine_readable"]["open_terms"]
            .as_sequence()
            .expect("open_terms");
        assert_eq!(terms.len(), 2, "the existing term must survive");
        assert_eq!(terms[0]["id"].as_str(), Some("standaardpremie"));
        assert_eq!(terms[1]["description"].as_str(), Some("redelijkerwijs"));
        assert_eq!(
            terms[1]["decided_per_case_by"].as_str(),
            Some("de Dienst Toeslagen")
        );
    }

    #[test]
    fn enables_is_dropped_and_reported() {
        let yaml = enriched(
            r#"      enables:
        - subject: de gemeente
          regulatory_layer: GEMEENTELIJKE_VERORDENING
"#,
        );
        let m = migrate(&yaml).expect("migrates");
        assert!(!m.yaml.contains("enables"));
        assert_eq!(
            m.dropped,
            vec![Dropped {
                article: "1".into(),
                field: "enables",
                count: 1,
            }]
        );
        assert!(m.schema_errors.is_empty(), "{:?}", m.schema_errors);
    }
}
