//! Generator for the precomputed implements-index
//! (`implements-index.json`) committed at the root of the corpus repo.
//!
//! Meant to be run by CI **in the corpus repo** against its own clean
//! checkout, on every branch that carries corpus content:
//!
//! ```text
//! # regenerate (writes <checkout>/implements-index.json):
//! implements-indexer <checkout>
//!
//! # verify the committed index matches the checkout (PR gate):
//! implements-indexer --check <checkout>
//! ```
//!
//! The generator refuses to run on a dirty scan subtree (the recorded tree
//! sha would not describe what was scanned). Individual unparseable laws
//! are reported and left out of the index; a systematic parse failure is
//! fatal. See [`regelrecht_corpus::implements_index::generator`].
//!
//! Exit codes: 0 = success / in sync; 1 = `--check` drift or missing
//! index; 2 = any other failure (dirty tree, git errors, systematic parse
//! breakage).

use std::path::PathBuf;
use std::process::ExitCode;

use regelrecht_corpus::implements_index::IMPLEMENTS_INDEX_FILENAME;
use regelrecht_corpus::implements_index::generator::{
    Args, DEFAULT_SCAN_ROOT, Drift, Outcome, Skipped, run,
};

/// Cap on how many failing paths are listed individually, so a noisy
/// corpus does not bury the actual result in CI output.
const MAX_LISTED_FAILURES: usize = 20;

fn parse_args() -> Result<Args, String> {
    let mut repo_root = None;
    let mut scan_root = DEFAULT_SCAN_ROOT.to_string();
    let mut check = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--root" => {
                scan_root = args
                    .next()
                    .ok_or_else(|| "--root requires a value".to_string())?;
            }
            "--help" | "-h" => {
                return Err(format!(
                    "usage: implements-indexer [--check] [--root <subtree>] <repo-root>\n\
                     \n\
                     Scans <repo-root>/<subtree> (default: {DEFAULT_SCAN_ROOT}) for \
                     .yaml/.yml law files and writes {IMPLEMENTS_INDEX_FILENAME} at \
                     <repo-root>. With --check, verifies the committed index instead."
                ));
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            other => {
                if repo_root.replace(PathBuf::from(other)).is_some() {
                    return Err("expected exactly one <repo-root> argument".to_string());
                }
            }
        }
    }

    Ok(Args {
        repo_root: repo_root.ok_or_else(|| "missing <repo-root> argument".to_string())?,
        scan_root,
        check,
    })
}

/// Report the laws that were left out. They fall through to a per-law
/// fetch at read time, so this is a corpus-quality signal, not an error.
fn report_skipped(skipped: &Skipped) {
    if skipped.is_empty() {
        return;
    }
    eprintln!(
        "warning: {} file(s) could not be parsed and were left out of the index \
         (readers fetch and parse those laws individually):",
        skipped.len()
    );
    for failure in skipped.iter().take(MAX_LISTED_FAILURES) {
        eprintln!("  {}: {}", failure.path, failure.error);
    }
    if skipped.len() > MAX_LISTED_FAILURES {
        eprintln!("  ... and {} more", skipped.len() - MAX_LISTED_FAILURES);
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    match run(&args) {
        Ok(Outcome::Wrote {
            path,
            files,
            declaring,
            bytes,
            tree_sha,
            skipped,
        }) => {
            report_skipped(&skipped);
            println!(
                "wrote {}: {files} files ({declaring} declaring implements), {bytes} bytes, \
                 tree {tree_sha}",
                path.display()
            );
            ExitCode::SUCCESS
        }
        Ok(Outcome::InSync {
            files,
            declaring,
            tree_sha,
            skipped,
        }) => {
            report_skipped(&skipped);
            println!(
                "index in sync: {files} files ({declaring} declaring implements), tree {tree_sha}"
            );
            ExitCode::SUCCESS
        }
        Ok(Outcome::Drifted(Drift::Missing { path })) => {
            eprintln!(
                "check failed: {} does not exist; run implements-indexer to generate it",
                path.display()
            );
            ExitCode::FAILURE
        }
        Ok(Outcome::Drifted(Drift::OutOfDate {
            path,
            committed_tree,
            checkout_tree,
        })) => {
            eprintln!(
                "check failed: {} is out of date with the checkout \
                 (committed tree {committed_tree}, checkout tree {checkout_tree}); regenerate it",
                path.display()
            );
            ExitCode::FAILURE
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}
