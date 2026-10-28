//! YAML writer for law files.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::text::{
    classify_text_style, normalize_text, should_wrap_text, wrap_text_default, TextStyle,
};
use crate::config::SCHEMA_URL;
use crate::error::{HarvesterError, Result};
use crate::types::{Law, Reference};

/// Regex matching a single-quoted YAML scalar value on a key line.
/// Captures: (1) prefix including key and colon-space, (2) the unquoted value.
#[allow(clippy::expect_used)]
static QUOTED_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*(?:- )?[a-zA-Z_$][a-zA-Z_$0-9]*: )'([^']*)'$").expect("valid regex")
});

/// Regex matching an unquoted YAML scalar value on a key line.
/// Excludes block scalar indicators (`|`, `>`) to avoid quoting them.
/// Captures: (1) prefix including key and colon-space, (2) the plain value.
#[allow(clippy::expect_used)]
static UNQUOTED_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"^(\s*(?:- )?[a-zA-Z_$][a-zA-Z_$0-9]*: )([^\s'"#\[{|>][^\n]*)$"##)
        .expect("valid regex")
});

/// Preamble representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlPreamble {
    text: String,
    url: String,
}

/// Article representation for YAML serialization.
///
/// References are serialized straight from the canonical [`Reference`] model
/// (`crate::types`) — it already carries the exact field set, order and
/// `skip_serializing_if` behavior, so there is no separate write mirror to keep
/// in sync.
#[derive(Debug, Serialize)]
struct YamlArticle {
    number: String,
    text: String,
    url: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    references: Vec<Reference>,
}

/// Full law representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlLaw {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "$id")]
    id: String,
    regulatory_layer: String,
    publication_date: String,
    valid_from: String,
    /// Instrument end date (inclusive). Emitted only when the law is terminated
    /// (vervalt/ingetrokken — RFC-019); otherwise omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bwb_id: Option<String>,
    // NOTE: cvdr_id is not yet in the schema; emitted as an extension for traceability.
    #[serde(skip_serializing_if = "Option::is_none")]
    cvdr_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    officiele_titel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organisation: Option<String>,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preamble: Option<YamlPreamble>,
    articles: Vec<YamlArticle>,
}

/// Normalize and wrap a text field, recording the emitted text and its block
/// scalar style for every multi-line emission in `fold_plan` (document order).
///
/// The stored `String` is the exact emitted (wrapped) text; [`fold_text_blocks`]
/// aligns it against the `text: |-` blocks serde actually emitted.
fn prepare_text(raw: &str, fold_plan: &mut Vec<(String, TextStyle)>) -> String {
    // First normalize the text to fix typographical issues from source XML
    let normalized = normalize_text(raw);

    // Then wrap if needed
    let text = if should_wrap_text(&normalized) {
        wrap_text_default(&normalized)
    } else {
        normalized.clone()
    };

    // Multi-line strings usually serialize as block scalars; record the emitted
    // text plus which style fold_text_blocks should apply. serde may instead
    // emit a quoted single-line scalar (e.g. when the text has an interior tab
    // or control character); such entries simply find no matching block and are
    // skipped during alignment.
    if text.contains('\n') {
        fold_plan.push((text.clone(), classify_text_style(&normalized, &text)));
    }
    text
}

/// Generate a schema-compliant YAML structure from a Law object, plus the
/// fold plan: one `(emitted_text, TextStyle)` per multi-line text field in
/// document order (preamble first, then articles — matching serde's emission
/// order).
fn generate_yaml_struct(law: &Law, effective_date: &str) -> (YamlLaw, Vec<(String, TextStyle)>) {
    let law_id = law.metadata.to_slug();
    let is_cvdr = law.metadata.cvdr_id.is_some();
    let mut fold_plan: Vec<(String, TextStyle)> = Vec::new();

    // Convert preamble if present (normalize and wrap like articles)
    let preamble = law.preamble.as_ref().map(|p| YamlPreamble {
        text: prepare_text(&p.text, &mut fold_plan),
        url: p.url.clone(),
    });

    let articles: Vec<YamlArticle> = law
        .articles
        .iter()
        .map(|article| YamlArticle {
            number: article.number.clone(),
            text: prepare_text(&article.text, &mut fold_plan),
            url: article.url.clone(),
            references: article.references.clone(),
        })
        .collect();

    // Build URL and IDs based on source type
    let (bwb_id, cvdr_id, officiele_titel, organisation, url) = if is_cvdr {
        let cvdr_id_str = law.metadata.cvdr_id.as_deref().unwrap_or_default();
        let url = format!("https://lokaleregelgeving.overheid.nl/{cvdr_id_str}");
        (
            None,
            law.metadata.cvdr_id.clone(),
            Some(law.metadata.title.clone()),
            law.metadata.creator.clone(),
            url,
        )
    } else {
        (
            Some(law.metadata.bwb_id.clone()),
            None,
            None,
            None,
            format!(
                "https://wetten.overheid.nl/{}/{}",
                law.metadata.bwb_id, effective_date
            ),
        )
    };

    let yaml_law = YamlLaw {
        schema: SCHEMA_URL.to_string(),
        id: law_id,
        regulatory_layer: law.metadata.regulatory_layer.as_str().to_string(),
        publication_date: law
            .metadata
            .publication_date
            .clone()
            .unwrap_or_else(|| effective_date.to_string()),
        valid_from: effective_date.to_string(),
        valid_to: law.metadata.valid_to.clone(),
        bwb_id,
        cvdr_id,
        officiele_titel,
        organisation,
        url,
        preamble,
        articles,
    };
    (yaml_law, fold_plan)
}

/// Regex matching the end of a block scalar header line (the value is a literal
/// `|` or folded `>` indicator, with an optional explicit indentation digit and
/// an optional chomping indicator). Applied to the *trimmed* line, e.g.
/// `text: |-`, `text: >`, `text: |2-`, `foo: >+`. The `: ` anchor keeps a plain
/// scalar that merely ends in `|`/`>` (which YAML would quote anyway) from
/// matching.
#[allow(clippy::expect_used)]
static BLOCK_SCALAR_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r": [|>]\d*[+-]?$").expect("valid regex"));

/// Indent YAML sequences to comply with `indent-sequences: true`.
///
/// serde_yaml_ng places sequence items (`- `) at the same indent as their parent key.
/// This function adds 2 spaces so items are indented under their parent, e.g.:
///
/// ```yaml
/// # Before:          # After:
/// articles:          articles:
/// - number: '1'        - number: '1'
///   text: foo            text: foo
/// ```
///
/// It is block-scalar-aware: content lines inside a `|`/`>` block are shifted by
/// exactly the same amount as their header line and are NOT interpreted as
/// sequence items, even when they start with `- `. Treating a `- ` block-content
/// line as a sequence item would over-indent it non-uniformly (only dashed lines
/// shift), corrupting the literal text and breaking [`fold_text_blocks`]'s
/// min-indent reconstruction.
fn indent_yaml_sequences(yaml: &str) -> String {
    let mut result: Vec<String> = Vec::new();
    // Stack of indent levels where sequences start
    let mut seq_indents: Vec<usize> = Vec::new();
    // When inside a block scalar: (header's original indent, the extra shift
    // frozen at the header line). Content lines reuse this shift verbatim.
    let mut block: Option<(usize, usize)> = None;

    for line in yaml.lines() {
        let trimmed = line.trim_start();

        // Pass empty lines through unchanged (blank lines inside a block scalar
        // keep the block open — they carry no indentation to compare).
        if trimmed.is_empty() {
            result.push(line.to_string());
            continue;
        }

        let indent = line.len() - trimmed.len();

        // Inside a block scalar: a line indented deeper than the header is block
        // content — emit it with the frozen shift, never as a sequence item. A
        // line at or below the header's indent ends the block; fall through to
        // normal processing.
        if let Some((header_indent, frozen_extra)) = block {
            if indent > header_indent {
                if frozen_extra > 0 {
                    result.push(format!("{}{}", " ".repeat(indent + frozen_extra), trimmed));
                } else {
                    result.push(line.to_string());
                }
                continue;
            }
            block = None;
        }

        // Pop sequences we've exited: either moved to a shallower indent,
        // or returned to the same indent but not as a sequence continuation.
        while let Some(&seq_indent) = seq_indents.last() {
            if indent < seq_indent || (indent == seq_indent && !trimmed.starts_with("- ")) {
                seq_indents.pop();
            } else {
                break;
            }
        }

        // Detect new or continuing sequence
        if trimmed.starts_with("- ") {
            let is_continuation = seq_indents.last().is_some_and(|&si| si == indent);
            if !is_continuation {
                seq_indents.push(indent);
            }
        }

        // Apply extra indentation
        let extra = seq_indents.len() * 2;

        // Detect a block scalar header (`key: |`, `key: >-`, …). The header line
        // itself is indented normally below; subsequent deeper lines are block
        // content shifted by this same `extra`.
        if BLOCK_SCALAR_HEADER_RE.is_match(trimmed) {
            block = Some((indent, extra));
        }

        if extra > 0 {
            result.push(format!("{}{}", " ".repeat(indent + extra), trimmed));
        } else {
            result.push(line.to_string());
        }
    }

    result.join("\n")
}

/// Check if a plain YAML scalar would be parsed as a non-string type.
///
/// Returns `true` if the value needs single quotes to remain a string
/// (integers, floats, dates, booleans, null, or values with special characters).
fn needs_yaml_quoting(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }

    // YAML booleans and null
    match value.to_lowercase().as_str() {
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~" => return true,
        _ => {}
    }

    // Starts with YAML special character
    if let Some(&first) = value.as_bytes().first() {
        if b"{}[],&*#?|-<>=!%@:\"`' ".contains(&first) {
            return true;
        }
    }

    // Contains problematic sequences or trailing colon
    if value.contains(": ") || value.contains(" #") || value.ends_with(':') {
        return true;
    }

    let num_part = value.strip_prefix('-').unwrap_or(value);

    // Pure integer
    if !num_part.is_empty() && num_part.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }

    // Float: digits.digits (exactly one dot, digits on both sides)
    if let Some(dot_pos) = num_part.find('.') {
        let (before, after_with_dot) = num_part.split_at(dot_pos);
        let after = &after_with_dot[1..];
        if !before.is_empty()
            && !after.is_empty()
            && before.bytes().all(|b| b.is_ascii_digit())
            && after.bytes().all(|b| b.is_ascii_digit())
        {
            return true;
        }
    }

    // Scientific-notation float, e.g. "24e1", "1e5", "2E-3", "1.5e3". The dotted
    // check above misses these (there is no dot, or a dot *plus* an exponent),
    // yet serde_yaml_ng resolves them to floats on read — so without quotes an
    // article number like "24e1" round-trips into the float 240.0 and breaks the
    // schema's `string` requirement. The mantissa may carry a single optional
    // dot; the exponent is digits with an optional sign.
    if let Some((mantissa, exponent)) = num_part.split_once(['e', 'E']) {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        let mantissa_ok = mantissa.bytes().any(|b| b.is_ascii_digit())
            && mantissa.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            && mantissa.bytes().filter(|&b| b == b'.').count() <= 1;
        let exponent_ok = !exponent.is_empty() && exponent.bytes().all(|b| b.is_ascii_digit());
        if mantissa_ok && exponent_ok {
            return true;
        }
    }

    // Date: YYYY-MM-DD
    let date_parts: Vec<&str> = value.split('-').collect();
    if date_parts.len() == 3
        && date_parts[0].len() == 4
        && date_parts[1].len() == 2
        && date_parts[2].len() == 2
        && date_parts
            .iter()
            .all(|p| p.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }

    false
}

/// Fix YAML scalar quoting to match yamllint's `quoted-strings: {required: only-when-needed}`.
///
/// - Strips quotes from values that YAML would parse as strings anyway.
/// - Adds quotes to unquoted values that would be misinterpreted (dates, booleans, numbers).
fn fix_yaml_quoting(yaml: &str) -> String {
    yaml.lines()
        .map(|line| {
            // Try to strip redundant quotes from quoted values
            if let Some(caps) = QUOTED_VALUE_RE.captures(line) {
                let (Some(prefix), Some(value)) = (caps.get(1), caps.get(2)) else {
                    return line.to_string();
                };
                let value = value.as_str();
                if needs_yaml_quoting(value) {
                    line.to_string()
                } else {
                    format!("{}{value}", prefix.as_str())
                }
            }
            // Try to add missing quotes to unquoted values that need them
            else if let Some(caps) = UNQUOTED_VALUE_RE.captures(line) {
                let (Some(prefix), Some(value)) = (caps.get(1), caps.get(2)) else {
                    return line.to_string();
                };
                let value = value.as_str();
                if needs_yaml_quoting(value) {
                    format!("{}'{value}'", prefix.as_str())
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Regex matching the header line of a plain multi-line `text:` block scalar as
/// serde_yaml_ng emits it (`text: |-`). Headers with an explicit indentation
/// indicator (`text: |N-`, emitted when the first content line starts with
/// whitespace) deliberately do NOT match: such blocks always classify as
/// `Literal` anyway, so they are passed through verbatim without consuming a
/// plan entry. The clip form (`text: |`, emitted for text ending in a
/// newline) is excluded for the same reason — a safe literal pass-through.
#[allow(clippy::expect_used)]
static TEXT_BLOCK_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)text: \|-$").expect("valid regex"));

/// Rewrite `text: |-` literal blocks to folded (`>-`) blocks per the fold plan.
///
/// The plan is produced by [`generate_yaml_struct`]: one `(emitted_text, style)`
/// entry per multi-line text field in document order. The emitted `text: |-`
/// blocks are an *ordered subsequence* of that plan: serde emits most multi-line
/// texts as a `|-` block, but chooses a double-quoted single-line scalar for a
/// text that contains an interior tab or other control character — that field
/// then appears in the plan with no corresponding block.
///
/// Alignment is therefore positional + content-based: for each `|-` block we
/// reconstruct its original text and advance a plan pointer, skipping any plan
/// entries whose text does not match (those were emitted as quoted scalars).
/// A block that matches no remaining plan entry means the emission invariant
/// broke and we fail loudly (`FoldPlanMismatch`) rather than guess which style
/// applies. Leftover plan entries after the scan are legal — they are the
/// quoted-scalar emissions.
///
/// Folded semantics: a single line break loads as a space (undoing the
/// cosmetic wrap), and N consecutive breaks load as N-1 newlines — so each
/// blank line (a `\n\n` paragraph break in the source) must become two blank
/// lines to survive the round trip. [`classify_text_style`] only marks a
/// block `Folded` when its content makes that transformation exact.
///
/// Note: a trailing newline in `yaml` is not preserved in the output; the
/// caller is responsible for any required terminal newline.
fn fold_text_blocks(yaml: &str, plan: &[(String, TextStyle)]) -> Result<String> {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + plan.len());
    let mut k = 0usize;
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];
        let Some(caps) = TEXT_BLOCK_HEADER_RE.captures(line) else {
            out.push(line.to_string());
            i += 1;
            continue;
        };
        let key_indent = caps.get(1).map_or(0, |m| m.as_str().len());

        // Collect the block: lines that are blank or indented deeper than the key.
        let mut end = i + 1;
        while end < lines.len() {
            let l = lines[end];
            let trimmed = l.trim_start();
            if !trimmed.is_empty() && l.len() - trimmed.len() <= key_indent {
                break;
            }
            end += 1;
        }
        let block_lines = &lines[i + 1..end];

        // Reconstruct the original emitted text. The block indentation is the
        // minimum leading whitespace over the non-blank lines — exactly what a
        // YAML parser auto-detects. `indent_yaml_sequences` shifts every block
        // line (dashed or not) by the same amount, so the content is uniformly
        // indented here; stripping the detected minimum recovers the wrapped
        // text, and any genuinely deeper-indented source line keeps its extra
        // spaces. Blank lines → empty string; join with '\n' (chomping `-`: no
        // trailing newline).
        // Stripping the minimum indent recovers the emitted text exactly
        // because indent_yaml_sequences shifts all content lines of a block
        // uniformly (frozen at the header). If that invariant ever breaks,
        // reconstruction diverges and the alignment below fails loudly.
        let block_indent = block_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(key_indent + 2);
        let reconstructed = block_lines
            .iter()
            .map(|l| {
                if l.trim().is_empty() {
                    ""
                } else {
                    &l[block_indent..]
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Advance the plan pointer past quoted-scalar emissions until the text
        // matches; a block with no matching plan entry fails loudly.
        while k < plan.len() && plan[k].0 != reconstructed {
            k += 1;
        }
        let style =
            plan.get(k)
                .map(|(_, s)| *s)
                .ok_or_else(|| HarvesterError::FoldPlanMismatch {
                    detail: format!(
                    "serialized `text: |-` block did not match any remaining fold-plan entry: {:?}",
                    reconstructed.chars().take(60).collect::<String>()
                ),
                })?;
        k += 1;

        match style {
            TextStyle::Folded => {
                out.push(format!("{}text: >-", " ".repeat(key_indent)));
                for l in block_lines {
                    if l.trim().is_empty() {
                        // Paragraph break: fold semantics eat one line break,
                        // so a blank line must double to keep `\n\n`.
                        out.push(String::new());
                        out.push(String::new());
                    } else {
                        out.push((*l).to_string());
                    }
                }
            }
            TextStyle::Literal => {
                out.push(line.to_string());
                out.extend(block_lines.iter().map(|l| l.to_string()));
            }
        }
        i = end;
    }

    Ok(out.join("\n"))
}

/// Generate YAML string from a Law object.
pub fn generate_yaml(law: &Law, effective_date: &str) -> Result<String> {
    let (yaml_struct, fold_plan) = generate_yaml_struct(law, effective_date);
    let yaml_string = serde_yaml_ng::to_string(&yaml_struct)?;

    // Post-process for yamllint compliance
    let yaml_string = fix_yaml_quoting(&yaml_string);
    let yaml_string = indent_yaml_sequences(&yaml_string);
    let yaml_string = fold_text_blocks(&yaml_string, &fold_plan)?;

    // Add document start marker and clean up trailing whitespace
    let lines: Vec<&str> = yaml_string.lines().map(|l| l.trim_end()).collect();
    let content = format!("---\n{}\n", lines.join("\n"));

    Ok(content)
}

/// Save a Law object as a YAML file.
///
/// Uses atomic write pattern: writes to temp file, syncs to disk, then renames.
/// This ensures partial writes don't corrupt existing files on crash.
///
/// # Arguments
/// * `law` - The Law object to save
/// * `effective_date` - The effective date in YYYY-MM-DD format
/// * `output_base` - Base directory for output (default: "regulation/nl/")
///
/// # Returns
/// Path to the saved file
pub fn save_yaml(law: &Law, effective_date: &str, output_base: Option<&Path>) -> Result<PathBuf> {
    let output_base = output_base.unwrap_or(Path::new("regulation/nl"));

    // Determine directory structure
    let layer_dir = law.metadata.regulatory_layer.as_dir_name();
    let law_id = law.metadata.to_slug();
    let output_dir = output_base.join(layer_dir).join(&law_id);
    fs::create_dir_all(&output_dir)?;

    let output_file = output_dir.join(format!("{effective_date}.yaml"));
    let temp_file = output_dir.join(format!(".{effective_date}.yaml.tmp"));

    // Generate YAML content
    let content = generate_yaml(law, effective_date)?;

    // Write to temp file first, then sync and rename for atomicity
    {
        let mut file = File::create(&temp_file)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?; // Ensure data is flushed to disk
    }

    // On Windows, rename fails if the destination already exists
    #[cfg(target_os = "windows")]
    if output_file.exists() {
        fs::remove_file(&output_file)?;
    }

    // Atomic rename (on most filesystems)
    fs::rename(&temp_file, &output_file)?;

    Ok(output_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Article, LawMetadata, RegulatoryLayer};
    use tempfile::tempdir;

    fn create_test_law() -> Law {
        let metadata = LawMetadata {
            bwb_id: "BWBR0018451".to_string(),
            cvdr_id: None,
            title: "Wet op de zorgtoeslag".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: Some("2005-12-29".to_string()),
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };

        let mut law = Law::new(metadata);
        law.add_article(Article::new(
            "1",
            "In deze wet wordt verstaan onder toeslagpartner: partner.",
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1",
        ));
        law
    }

    #[test]
    fn test_save_yaml() {
        let law = create_test_law();
        let temp_dir = tempdir().unwrap();
        let output_path = save_yaml(&law, "2025-01-01", Some(temp_dir.path())).unwrap();

        assert!(output_path.exists());
        // Check path components (works on both Windows and Unix)
        let path_str = output_path.to_string_lossy();
        assert!(path_str.contains("wet"));
        assert!(path_str.contains("wet_op_de_zorgtoeslag"));
        assert!(path_str.contains("2025-01-01.yaml"));

        let content = fs::read_to_string(output_path).unwrap();
        assert!(content.starts_with("---\n"));
    }

    #[test]
    fn test_generate_yaml_no_redundant_quotes() {
        let mut law = create_test_law();
        law.add_article(Article::new(
            "1.1.a",
            "Sub-article text",
            "https://example.com",
        ));
        let yaml = generate_yaml(&law, "2025-01-01").unwrap();

        // 1.1.a should NOT be quoted (contains letters, not a number)
        assert!(
            yaml.contains("number: 1.1.a"),
            "1.1.a should not be quoted, got: {}",
            yaml
        );
        // But dates should remain quoted
        assert!(
            yaml.contains("publication_date: '2005-12-29'"),
            "Dates should remain quoted"
        );
    }

    #[test]
    fn test_needs_yaml_quoting() {
        // Values that need quoting
        assert!(needs_yaml_quoting("1.1")); // float
        assert!(needs_yaml_quoting("42")); // integer
        assert!(needs_yaml_quoting("2024-10-16")); // date
        assert!(needs_yaml_quoting("true")); // boolean
        assert!(needs_yaml_quoting("null")); // null
        assert!(needs_yaml_quoting("")); // empty
        assert!(needs_yaml_quoting("foo: bar")); // contains ": "
        assert!(needs_yaml_quoting("end:")); // ends with ":"
                                             // Scientific-notation floats: YAML resolves these to numbers on read,
                                             // so a string like the article number "24e1" must stay quoted or it
                                             // round-trips into the float 240.0.
        assert!(needs_yaml_quoting("24e1"));
        assert!(needs_yaml_quoting("1e5"));
        assert!(needs_yaml_quoting("2E10"));
        assert!(needs_yaml_quoting("2E-3"));
        assert!(needs_yaml_quoting("1.5e3"));

        // Values that don't need quoting
        assert!(!needs_yaml_quoting("1.1.a"));
        assert!(!needs_yaml_quoting("68b"));
        assert!(!needs_yaml_quoting("18d"));
        assert!(!needs_yaml_quoting("3.3.1"));
        assert!(!needs_yaml_quoting("4a.1"));
        assert!(!needs_yaml_quoting("ref1")); // 'e' inside, but no numeric mantissa
        assert!(!needs_yaml_quoting("BWBR0018451"));
        assert!(!needs_yaml_quoting("hello"));
        assert!(!needs_yaml_quoting("24e")); // trailing 'e', no exponent → stays a string
        assert!(!needs_yaml_quoting("e5")); // no mantissa digit → stays a string
    }

    #[test]
    fn test_indent_yaml_sequences() {
        let input =
            "top: val\nitems:\n- name: a\n  val: 1\n- name: b\n  nested:\n  - id: x\n    v: 1";
        let result = indent_yaml_sequences(input);
        assert_eq!(
            result,
            "top: val\nitems:\n  - name: a\n    val: 1\n  - name: b\n    nested:\n      - id: x\n        v: 1"
        );
    }

    #[test]
    fn test_indent_yaml_sequences_block_scalar_content() {
        // A literal block whose content mixes a non-dashed line, a blank line,
        // and `- ` lines must be shifted uniformly by the enclosing sequence's
        // extra (+2) — the `- ` content lines must NOT be treated as sequence
        // items (which would over-indent them to +4).
        let input =
            "articles:\n- number: '1'\n  text: |-\n    intro:\n\n    - item een\n    - item twee\n  url: x";
        let result = indent_yaml_sequences(input);
        assert_eq!(
            result,
            "articles:\n  - number: '1'\n    text: |-\n      intro:\n\n      - item een\n      - item twee\n    url: x"
        );
    }

    #[test]
    fn test_fix_yaml_quoting() {
        let input = "number: '1.1.a'\ndate: '2024-10-16'\nartikel: '68b'\ncount: '1'";
        let result = fix_yaml_quoting(input);
        assert_eq!(
            result,
            "number: 1.1.a\ndate: '2024-10-16'\nartikel: 68b\ncount: '1'"
        );
    }

    #[test]
    fn test_fix_yaml_quoting_scientific_notation_article_number() {
        // An article number like "24e1" (WVO, BWBR0002399) parses as the float
        // 240.0 unless quoted. Whether serde emits it quoted or bare, the pass
        // must leave it quoted so the value round-trips as a string.
        assert_eq!(fix_yaml_quoting("number: 24e1"), "number: '24e1'");
        assert_eq!(fix_yaml_quoting("artikel: 24e1"), "artikel: '24e1'");
        assert_eq!(fix_yaml_quoting("number: '24e1'"), "number: '24e1'");

        // And the quoted value survives a real serde_yaml_ng round trip as a string.
        let parsed: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fix_yaml_quoting("number: 24e1")).unwrap();
        assert_eq!(parsed.get("number").and_then(|v| v.as_str()), Some("24e1"));
    }

    /// A CVDR (local-regulation) law that exercises the writer-only projection
    /// fields — `preamble`, `organisation`, `cvdr_id`, `officiele_titel` and a
    /// per-article `references` block — which the canonical executable model
    /// (`ArticleBasedLaw`) does not carry. Keeping these in a golden test guards
    /// against accidental loss when the writer changes.
    fn create_test_cvdr_law() -> Law {
        let metadata = LawMetadata {
            bwb_id: String::new(),
            cvdr_id: Some("CVDR123456".to_string()),
            title: "Voorbeeldverordening".to_string(),
            regulatory_layer: RegulatoryLayer::GemeentelijkeVerordening,
            publication_date: Some("2024-01-01".to_string()),
            effective_date: None,
            valid_to: None,
            creator: Some("Gemeente Voorbeeld".to_string()),
            scope_code: Some("GM0000".to_string()),
        };

        let mut law = Law::new(metadata);
        law.preamble = Some(crate::types::Preamble {
            text: "De raad van de gemeente Voorbeeld besluit.".to_string(),
            url: "https://lokaleregelgeving.overheid.nl/CVDR123456".to_string(),
        });
        law.add_article(
            Article::new(
                "1",
                "Begripsbepalingen.",
                "https://lokaleregelgeving.overheid.nl/CVDR123456#Artikel1",
            )
            .with_references(vec![Reference {
                id: "ref1".to_string(),
                bwb_id: "BWBR0018451".to_string(),
                artikel: Some("4".to_string()),
                lid: None,
                onderdeel: None,
                hoofdstuk: None,
                paragraaf: None,
                afdeling: None,
            }]),
        );
        law
    }

    /// Golden byte-identity test for a national (BWB) law. Pins the *exact*
    /// serialized output so any change to the writer (formatting, field order,
    /// the de-duplicated `references` projection) is caught, not just substrings.
    #[test]
    fn test_generate_yaml_golden_bwb() {
        let law = create_test_law();
        let yaml = generate_yaml(&law, "2025-01-01").unwrap();
        // Indentation below is significant — it is the exact emitted YAML.
        // SCHEMA_URL is interpolated so a schema-version bump updates this golden
        // automatically while every other byte stays pinned.
        let expected = format!(
            "---
$schema: {SCHEMA_URL}
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2005-12-29'
valid_from: '2025-01-01'
bwb_id: BWBR0018451
url: https://wetten.overheid.nl/BWBR0018451/2025-01-01
articles:
  - number: '1'
    text: 'In deze wet wordt verstaan onder toeslagpartner: partner.'
    url: https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1
"
        );
        assert_eq!(yaml, expected);
    }

    /// Golden byte-identity test for a CVDR (local) law, covering the
    /// writer-only projection fields and a per-article `references` block.
    #[test]
    fn test_generate_yaml_golden_cvdr() {
        let law = create_test_cvdr_law();
        let yaml = generate_yaml(&law, "2024-01-01").unwrap();
        // Indentation below is significant — it is the exact emitted YAML,
        // including the writer-only `preamble`/`organisation`/`references` fields.
        let expected = format!(
            "---
$schema: {SCHEMA_URL}
$id: voorbeeldverordening
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2024-01-01'
valid_from: '2024-01-01'
cvdr_id: CVDR123456
officiele_titel: Voorbeeldverordening
organisation: Gemeente Voorbeeld
url: https://lokaleregelgeving.overheid.nl/CVDR123456
preamble:
  text: De raad van de gemeente Voorbeeld besluit.
  url: https://lokaleregelgeving.overheid.nl/CVDR123456
articles:
  - number: '1'
    text: Begripsbepalingen.
    url: https://lokaleregelgeving.overheid.nl/CVDR123456#Artikel1
    references:
      - id: ref1
        bwb_id: BWBR0018451
        artikel: '4'
"
        );
        assert_eq!(yaml, expected);
    }

    /// Conformance: the harvested YAML deserializes cleanly into the canonical
    /// executable model (`regelrecht-law-model::ArticleBasedLaw`). The harvester
    /// writer stays a separate write-projection, but this proves its output
    /// remains faithful to the single source of truth for the law format.
    #[test]
    fn test_generated_yaml_conforms_to_law_model() {
        for (law, date) in [
            (create_test_law(), "2025-01-01"),
            (create_test_cvdr_law(), "2024-01-01"),
            (create_test_folded_law(), "2026-01-01"),
        ] {
            let yaml = generate_yaml(&law, date).unwrap();
            let parsed: regelrecht_law_model::ArticleBasedLaw = serde_yaml_ng::from_str(&yaml)
                .unwrap_or_else(|e| {
                    panic!("harvested YAML must parse as ArticleBasedLaw: {e}\n{yaml}")
                });
            assert_eq!(parsed.articles.len(), law.articles.len());
            assert_eq!(parsed.regulatory_layer, law.metadata.regulatory_layer);
        }
    }

    /// Long multi-paragraph prose that must wrap (>115 chars) and fold back
    /// to the unwrapped source on load.
    const FOLDABLE_PROSE: &str = "Indien de normpremie voor een verzekerde in het berekeningsjaar minder bedraagt dan de standaardpremie in dat jaar, heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil.\n\nDe normpremie bedraagt een percentage van het drempelinkomen in het berekeningsjaar, vermeerderd met een percentage van het toetsingsinkomen van de verzekerde (artikel 2, tweede lid).";

    fn create_test_folded_law() -> Law {
        let mut law = create_test_law();
        law.add_article(Article::new(
            "2",
            FOLDABLE_PROSE,
            "https://wetten.overheid.nl/BWBR0018451/2026-01-01#Artikel2",
        ));
        law
    }

    /// Golden byte-identity test for a folded (`>-`) text block: wrapped
    /// lines, and a doubled blank line for the paragraph break so the
    /// `\n\n` survives the folded round trip.
    #[test]
    fn test_generate_yaml_golden_folded() {
        let law = create_test_folded_law();
        let yaml = generate_yaml(&law, "2026-01-01").unwrap();
        let expected = format!(
            "---
$schema: {SCHEMA_URL}
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2005-12-29'
valid_from: '2026-01-01'
bwb_id: BWBR0018451
url: https://wetten.overheid.nl/BWBR0018451/2026-01-01
articles:
  - number: '1'
    text: 'In deze wet wordt verstaan onder toeslagpartner: partner.'
    url: https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1
  - number: '2'
    text: >-
      Indien de normpremie voor een verzekerde in het berekeningsjaar minder bedraagt dan de standaardpremie in dat jaar,
      heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil.


      De normpremie bedraagt een percentage van het drempelinkomen in het berekeningsjaar, vermeerderd met een percentage
      van het toetsingsinkomen van de verzekerde (artikel 2, tweede lid).
    url: https://wetten.overheid.nl/BWBR0018451/2026-01-01#Artikel2
"
        );
        assert_eq!(yaml, expected);
    }

    /// Round-trip property over representative text shapes: emitting and
    /// re-parsing must yield exactly the expected string — the unwrapped
    /// source for foldable prose, the wrapped text for literal fallbacks.
    /// Also pins output hygiene: no 3+ consecutive blank lines anywhere, and
    /// no folded content line starting with whitespace.
    #[test]
    fn test_fold_round_trip_property() {
        let long_url = format!(
            "https://example.com/{}",
            "zeer-lange-url-component/".repeat(6)
        );
        struct Case {
            name: &'static str,
            source: &'static str,
            owned_source: Option<String>,
            /// None = round trip must return the source unchanged (folded or
            /// short). Some(f) = expected loaded text derived from the source.
            expect_wrapped: bool,
            expect_indicator: Option<&'static str>,
        }
        let ref_heavy_source = format!(
            "Zie [artikel 4 van de Wet op de zorgtoeslag][ref1] en [artikel 2.18 van de Wet inkomstenbelasting 2001][ref2] voor de berekening van de premie.\n\n[ref1]: {long_url}\n[ref2]: https://example.com/b\n[ref3]: https://example.com/c\n[ref4]: https://example.com/d"
        );
        let cases = [
            Case {
                name: "multi-paragraph unicode prose folds to unwrapped source",
                source: "Indien de normpremie voor een verzekerde in het berekeningsjaar minder bedraagt dan de standaardpremie (€ 1,50 per maand, 3\u{00b0} categorie) in dat jaar, heeft de verzekerd\u{00eb} aanspraak op een zorgtoeslag.\n\nDe pensioengerechtigde leeftijd, bedoeld in artikel 7a van de Algemene Ouderdomswet, wordt verhoogd met drie maanden per kalenderjaar tot de leeftijd van zeventig jaar is bereikt.",
                owned_source: None,
                expect_wrapped: false,
                expect_indicator: Some(">-"),
            },
            Case {
                name: "reference definitions stay literal, each on its own line",
                source: "",
                owned_source: Some(ref_heavy_source),
                expect_wrapped: true,
                expect_indicator: Some("|-"),
            },
            Case {
                name: "column-0 list stays literal",
                source: "- eerste onderdeel van de opsomming met voldoende lengte om de wrap-drempel te halen zodat er gewrapt wordt\n- tweede onderdeel van de opsomming met voldoende lengte om de wrap-drempel te halen",
                owned_source: None,
                expect_wrapped: true,
                expect_indicator: Some("|-"),
            },
            Case {
                name: "triple newline stays literal",
                source: "Eerste stuk tekst dat ruim voldoende lengte heeft om de wrap-drempel van tachtig tekens te overschrijden.\n\n\nTweede stuk tekst na een dubbele lege regel, eveneens lang genoeg om te wrappen.",
                owned_source: None,
                expect_wrapped: true,
                expect_indicator: Some("|-"),
            },
            Case {
                name: "short text stays a plain scalar",
                source: "Begripsbepalingen.",
                owned_source: None,
                expect_wrapped: false,
                expect_indicator: None,
            },
            Case {
                // Regression: a long single-paragraph text with an interior tab
                // makes serde emit a double-quoted single-line scalar instead of
                // a block. The fold plan then has an entry with no matching
                // block; content-subsequence alignment must skip it rather than
                // fail generation. serde's quoted scalar preserves the wrap
                // newlines, so the loaded text is the wrapped text.
                name: "long text with interior tab emits a quoted scalar",
                source: "Indien de normpremie voor een verzekerde in het berekeningsjaar minder bedraagt dan de\tstandaardpremie in dat jaar, heeft de verzekerde aanspraak op een zorgtoeslag.",
                owned_source: None,
                expect_wrapped: true,
                expect_indicator: None,
            },
            Case {
                // Regression: a chapeau/intro line followed by `- ` bullet lines
                // in a literal block. `indent_yaml_sequences` must NOT treat the
                // dashed content lines as sequence items — a non-uniform
                // over-indent would leave stray spaces after min-indent
                // reconstruction, breaking the fold-plan match and failing
                // generation entirely. The single `\n` between bullets keeps the
                // block Literal (`|-`). Loads back verbatim, bullets at column 0.
                name: "chapeau then dash list stays literal and round-trips exactly",
                source: "1 In deze regeling wordt verstaan onder:\n\n- *accountant:* een accountant als bedoeld in artikel 393 van Boek 2 van het Burgerlijk Wetboek en nog wat woorden om de tekst lang te maken;\n- *btw:* omzetbelasting",
                owned_source: None,
                expect_wrapped: true,
                expect_indicator: Some("|-"),
            },
        ];

        for case in &cases {
            let source: &str = case.owned_source.as_deref().unwrap_or(case.source);
            let mut law = create_test_law();
            law.add_article(Article::new("2", source, "https://example.com/#2"));
            let yaml = generate_yaml(&law, "2026-01-01").unwrap();
            let parsed: regelrecht_law_model::ArticleBasedLaw = serde_yaml_ng::from_str(&yaml)
                .unwrap_or_else(|e| panic!("case '{}': output must parse: {e}\n{yaml}", case.name));
            let loaded = &parsed.articles[1].text;

            let normalized = normalize_text(source);
            let expected = if case.expect_wrapped {
                wrap_text_default(&normalized)
            } else {
                normalized.clone()
            };
            assert_eq!(
                loaded, &expected,
                "case '{}': loaded text mismatch\n{yaml}",
                case.name
            );

            if let Some(indicator) = case.expect_indicator {
                assert!(
                    yaml.contains(&format!("text: {indicator}\n")),
                    "case '{}': expected `text: {indicator}` block\n{yaml}",
                    case.name
                );
            }

            // Reference definitions must each keep their own line after load.
            if source.contains("]: ") {
                for line in normalized.lines().filter(|l| l.starts_with("[ref")) {
                    assert!(
                        loaded.lines().any(|l| l == line),
                        "case '{}': ref definition line lost: {line}\n{loaded}",
                        case.name
                    );
                }
            }

            // Column-0 list content must survive without injected leading
            // spaces: `indent_yaml_sequences` used to over-indent `- ` block
            // lines (mistaking them for sequence items), which the min-indent
            // reconstruction then leaked back as stray leading spaces.
            if matches!(
                case.name,
                "chapeau then dash list stays literal and round-trips exactly"
                    | "column-0 list stays literal"
            ) {
                assert!(
                    !loaded.lines().any(|l| l.starts_with(' ')),
                    "case '{}': loaded text has an injected leading space\n{loaded}",
                    case.name
                );
            }

            // Output hygiene: yamllint limits (empty-lines max 2) and folded
            // blocks free of accidental more-indented lines.
            let lines: Vec<&str> = yaml.lines().collect();
            assert!(
                !lines.windows(3).any(|w| w.iter().all(|l| l.is_empty())),
                "case '{}': 3+ consecutive blank lines\n{yaml}",
                case.name
            );
        }
    }

    #[test]
    fn test_fold_text_blocks_plan_mismatch_errors() {
        // A `text: |-` block whose reconstructed content matches no plan entry
        // (here: empty plan) means the emission invariant broke — fail loudly.
        let yaml = "articles:\n  - number: '1'\n    text: |-\n      regel een\n      regel twee\n";
        let err = fold_text_blocks(yaml, &[]).unwrap_err();
        assert!(
            err.to_string().contains("fold plan mismatch"),
            "unexpected error: {err}"
        );

        // Plan entries with NO matching block are legal now: serde may emit a
        // text as a quoted scalar (tabs/control chars). Leftovers must NOT
        // error, and the input passes through unchanged.
        let input = "key: value\n";
        let out = fold_text_blocks(
            input,
            &[("some emitted\ntext".to_string(), TextStyle::Folded)],
        )
        .unwrap();
        assert_eq!(out, "key: value");

        // Misassignment safety: a block whose content is B cannot be silently
        // matched to a plan entry for A — it must fail rather than fold the
        // wrong block.
        let yaml_b = "text: |-\n  regel B\n  regel B twee\n";
        let err = fold_text_blocks(
            yaml_b,
            &[("regel A\nregel A twee".to_string(), TextStyle::Folded)],
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("fold plan mismatch"),
            "unexpected error: {err}"
        );
    }
}
