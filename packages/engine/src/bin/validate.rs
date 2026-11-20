use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};
use regelrecht_engine::schema::{detect_version, load_schemas, validation_errors};

/// A file that parsed, kept for the whole run so the type check can consult
/// the other laws of the set (rule N5).
struct Parsed {
    path: PathBuf,
    law: ArticleBasedLaw,
}

/// The version of each law the type check consults for cross-law rules: the
/// one with the latest `valid_from`, as the engine would select for a date
/// after every version. A law given twice with the same `valid_from` keeps
/// the first.
fn latest_by_id(parsed: &[Parsed]) -> HashMap<&str, &ArticleBasedLaw> {
    let mut latest: HashMap<&str, &ArticleBasedLaw> = HashMap::new();
    for entry in parsed {
        let law = &entry.law;
        match latest.get(law.id.as_str()) {
            Some(existing) if existing.valid_from >= law.valid_from => {}
            _ => {
                latest.insert(law.id.as_str(), law);
            }
        }
    }
    latest
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: validate <file1.yaml> [file2.yaml ...]");
        process::exit(1);
    }

    let schemas = match load_schemas() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FATAL: {e}");
            process::exit(2);
        }
    };
    let mut failed = false;

    // Step 1: serde deserialization check (catches type/structure errors).
    // Every file is parsed before any is checked, so the type check can hold
    // a law against the laws it references when those are in the same set.
    let mut parsed: Vec<Parsed> = Vec::new();
    for arg in &args {
        let path = Path::new(arg);
        match ArticleBasedLaw::from_yaml_file(path) {
            Ok(law) => parsed.push(Parsed {
                path: path.to_path_buf(),
                law,
            }),
            Err(e) => {
                eprintln!("FAIL: {}: serde: {e}", path.display());
                failed = true;
            }
        }
    }
    let latest = latest_by_id(&parsed);
    let lookup = |id: &str| latest.get(id).copied();

    for entry in &parsed {
        let path = entry.path.as_path();
        let law = &entry.law;

        // Step 2: JSON Schema validation
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FAIL: {}: read: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let value: serde_json::Value = match serde_yaml_ng::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("FAIL: {}: yaml parse: {e}", path.display());
                failed = true;
                continue;
            }
        };

        let version = detect_version(&value);
        let schema_ok = match version {
            Some(ver) => {
                let schema = &schemas[ver];
                match validation_errors(schema, &value) {
                    Ok(errors) if errors.is_empty() => {
                        eprintln!("OK: {} (schema {ver})", path.display());
                        true
                    }
                    Ok(errors) => {
                        eprintln!("FAIL: {}: schema ({ver})", path.display());
                        for error in &errors {
                            eprintln!("  - {error}");
                        }
                        failed = true;
                        false
                    }
                    Err(e) => {
                        eprintln!(
                            "FAIL: {}: could not compile schema {ver}: {e}",
                            path.display()
                        );
                        failed = true;
                        false
                    }
                }
            }
            None => {
                // Check if $schema field exists but version is unrecognized
                if value.get("$schema").is_some() {
                    eprintln!("FAIL: {}: unrecognized $schema version", path.display());
                } else {
                    eprintln!("FAIL: {}: missing $schema field", path.display());
                }
                failed = true;
                false
            }
        };

        // Step 3: RFC-023 static unit check — only when the schema validated, so
        // unit findings aren't interleaved with (and possibly artefacts of) a
        // schema error. Mismatches between declared units (e.g. eurocent + days)
        // are FAILs; an `amount` output with no declared unit is a WARN.
        if !schema_ok {
            continue;
        }
        for finding in regelrecht_engine::units::check_law(law) {
            let kind = if finding.is_error { "FAIL" } else { "WARN" };
            eprintln!(
                "{kind}: {}: units: article {} output '{}': {}",
                path.display(),
                finding.article,
                finding.output,
                finding.message
            );
            if finding.is_error {
                failed = true;
            }
        }

        // Step 4: static type check (RFC-036 nullability, RFC-037 typing).
        // Every finding is a FAIL: a rule that fires on a correct law is a bug
        // in the rule, and the corpus is kept typed by this gate.
        for finding in regelrecht_engine::typecheck::check_law(law, &lookup) {
            eprintln!(
                "FAIL: {}: typecheck: article {} {}: [{}] {}",
                path.display(),
                finding.article,
                finding.location,
                finding.rule,
                finding.message
            );
            failed = true;
        }
    }

    // Step 5: couplings between hooks and their hosts in the set that
    // cannot work as written, hosts (RFC-058) the hook never reaches, and
    // fillings that ask for a parameter their host does not hand on. A
    // WARN: the regulation still runs, and a host may be outside the set.
    let laws: Vec<&ArticleBasedLaw> = latest.values().copied().collect();
    let mut warnings = regelrecht_engine::hook_coupling_warnings(&laws);
    warnings.sort();
    for warning in warnings {
        eprintln!("WARN: hook coupling: {warning}");
    }

    if failed {
        process::exit(1);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn parsed(id: &str, valid_from: &str, marker: &str) -> Parsed {
        let yaml = format!(
            "$id: {id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\nvalid_from: '{valid_from}'\nname: {marker}\narticles: []\n"
        );
        Parsed {
            path: PathBuf::from(format!("{id}-{marker}.yaml")),
            law: ArticleBasedLaw::from_yaml_str(&yaml).unwrap(),
        }
    }

    #[test]
    fn latest_by_id_selects_the_latest_valid_from_and_keeps_the_first_on_a_tie() {
        let set = vec![
            parsed("wet", "2024-01-01", "oud"),
            parsed("wet", "2025-01-01", "eerste"),
            parsed("wet", "2025-01-01", "tweede"),
            parsed("andere", "2023-01-01", "enige"),
        ];
        let latest = latest_by_id(&set);
        assert_eq!(latest["wet"].name.as_deref(), Some("eerste"));
        assert_eq!(latest["andere"].name.as_deref(), Some("enige"));
    }
}
