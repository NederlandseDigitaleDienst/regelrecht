//! The corpus stays typed: the static type check (RFC-036 nullability,
//! RFC-037 typing) finds nothing in `corpus/regulation` with the reductions
//! in `corpus/cells`, or in the demo corpus `corpus/demo/regulation`.
//!
//! Every law of a corpus is loaded first, so the cross-law rule (N5) is held
//! against the version of each referenced law the engine would select, then
//! every law is checked. A finding here means either a law that has to be
//! annotated or guarded, or a rule that fires on a correct law; the message
//! names the law, the article, the place and the rule, so the reader can tell
//! which.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::article::{ArticleBasedLaw, LawLoad};
use regelrecht_engine::typecheck::{check_law, Finding};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every law file of a corpus in `dirs`, parsed. Dot-prefixed sidecars are
/// not laws, and neither is the identity of a cell (`cel.yaml`).
fn load_corpus(dirs: &[PathBuf]) -> Vec<(PathBuf, ArticleBasedLaw)> {
    let mut laws = Vec::new();
    for entry in dirs
        .iter()
        .flat_map(|dir| WalkDir::new(dir).into_iter().filter_map(Result::ok))
    {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !path.is_file()
            || !name.ends_with(".yaml")
            || name.starts_with('.')
            || name == "cel.yaml"
        {
            continue;
        }
        let law = ArticleBasedLaw::from_yaml_file(path)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
        laws.push((path.to_path_buf(), law));
    }
    assert!(!laws.is_empty(), "no laws under {dirs:?}");
    laws
}

/// The findings over a corpus, each prefixed with the file it came from.
fn findings_over(dirs: &[PathBuf]) -> Vec<String> {
    let laws = load_corpus(dirs);
    // The version the engine selects for a date after every version: the
    // latest `valid_from`.
    let mut latest: HashMap<&str, &ArticleBasedLaw> = HashMap::new();
    for (_, law) in &laws {
        match latest.get(law.id.as_str()) {
            Some(existing) if existing.valid_from >= law.valid_from => {}
            _ => {
                latest.insert(law.id.as_str(), law);
            }
        }
    }
    let lookup = |id: &str| latest.get(id).copied();
    let mut report = Vec::new();
    for (path, law) in &laws {
        for finding in check_law(law, &lookup) {
            report.push(format_finding(path, &finding));
        }
    }
    report
}

fn format_finding(path: &Path, finding: &Finding) -> String {
    let relative = path
        .strip_prefix(repo_root())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    format!("{relative}: {finding}")
}

#[test]
fn corpus_regulation_has_no_type_findings() {
    let findings = findings_over(&[
        repo_root().join("corpus/regulation"),
        repo_root().join("corpus/cells"),
    ]);
    assert!(
        findings.is_empty(),
        "{} type finding(s) in corpus/regulation and corpus/cells:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

#[test]
fn corpus_demo_has_no_type_findings() {
    let findings = findings_over(&[repo_root().join("corpus/demo/regulation")]);
    assert!(
        findings.is_empty(),
        "{} type finding(s) in corpus/demo/regulation:\n{}",
        findings.len(),
        findings.join("\n")
    );
}
