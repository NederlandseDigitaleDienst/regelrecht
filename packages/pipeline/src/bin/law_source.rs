//! Hold a corpus law file against the official BWB toestand.
//!
//! ```text
//! law-source [--cache <dir>] [--offline] <file.yaml>...
//! ```
//!
//! Exits 1 when any article drifts, is missing or is fabricated, so it can
//! gate enrichment. This runs *before* the agent and outside it: the agent
//! has no network, and verifying the source is harvesting work. It stands
//! here until the harvester takes it back.
//!
//! `--cache` keeps the fetched XML so a second run costs nothing;
//! `--offline` uses only what is cached. `--rewrite` replaces the file's
//! `text` fields with the official text, drops articles the law does not
//! have, adds the ones it does, and writes the document structure to a
//! sidecar beside the file. Existing `machine_readable` is carried over by
//! article number: discarding it would throw away work, and the checks that
//! run afterwards are what decide whether the translation still holds
//! against the corrected text. A rewrite that would empty the file or
//! remove most of it is refused: that shape means the source is wrong, not
//! the law.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use regelrecht_pipeline::enrich_v2::source_gate::{
    CONTEXT_SIDECAR, Verdict, parse_toestand, rewrite, toestand_url, verify, write_atomic_pair,
};

#[tokio::main]
async fn main() -> ExitCode {
    let mut cache: Option<PathBuf> = None;
    let mut offline = false;
    let mut do_rewrite = false;
    let mut files: Vec<PathBuf> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cache" => match args.next() {
                Some(v) => cache = Some(PathBuf::from(v)),
                None => {
                    eprintln!("--cache requires a path");
                    return ExitCode::from(2);
                }
            },
            "--offline" => offline = true,
            "--rewrite" => do_rewrite = true,
            "--help" | "-h" => {
                println!(
                    "usage: law-source [--cache <dir>] [--offline] [--rewrite] <file.yaml>..."
                );
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option: {other}");
                return ExitCode::from(2);
            }
            other => files.push(PathBuf::from(other)),
        }
    }

    if files.is_empty() {
        eprintln!("usage: law-source [--cache <dir>] [--offline] [--rewrite] <file.yaml>...");
        return ExitCode::from(2);
    }

    let mut failed = 0usize;
    for file in &files {
        match check_one(file, cache.as_deref(), offline, do_rewrite).await {
            Ok(true) => {}
            Ok(false) => failed += 1,
            Err(e) => {
                println!("\n=== {}", file.display());
                println!("  ERROR: {e}");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        println!(
            "\n{failed} of {} file(s) did not pass the source gate.",
            files.len()
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn check_one(
    path: &Path,
    cache: Option<&Path>,
    offline: bool,
    do_rewrite: bool,
) -> Result<bool, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&raw).map_err(|e| format!("not YAML: {e}"))?;

    let bwb_id = doc
        .get("bwb_id")
        .and_then(serde_yaml_ng::Value::as_str)
        .ok_or("no bwb_id in the file")?;
    // Some corpus files carry no `valid_from`. The file name is the
    // toestand date by convention, so fall back to it and say so, rather
    // than refusing to check the file at all.
    let from_field = doc
        .get("valid_from")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_string);
    let valid_from = match from_field {
        Some(v) => v,
        None => {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("no valid_from and no usable file name")?;
            println!("\n  note: no valid_from in the file; using the file name {stem}");
            stem.to_string()
        }
    };
    let valid_from = valid_from.as_str();

    let xml = fetch(bwb_id, valid_from, cache, offline).await?;
    let official = parse_toestand(&xml)?;
    let report = verify(&doc, &official);

    println!("\n=== {}", path.display());
    println!(
        "  {bwb_id} @ {valid_from}, {} article(s) in the source",
        official.len()
    );

    let counts = report.counts();
    let summary: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!("  {}", summary.join(" "));

    for (number, verdict) in &report.verdicts {
        match verdict {
            Verdict::Verified => {}
            Verdict::Fabricated => {
                println!("    [fabricated] art. {number}: not in the official toestand at all")
            }
            Verdict::Missing => {
                println!("    [missing] art. {number}: in the law, absent from the file");
            }
            Verdict::Drift { detail } => println!("    [drift] art. {number}: {detail}"),
        }
    }

    // The placement the harvester drops on the floor. Printed even when the
    // gate passes, because it is what the enricher is missing.
    let placed: Vec<_> = report
        .placements
        .iter()
        .filter(|(_, p)| !p.is_empty())
        .collect();
    if !placed.is_empty() {
        println!(
            "  structure available for {} of {} articles, none of it in the corpus file:",
            placed.len(),
            report.placements.len()
        );
        for (number, placement) in placed.iter().take(6) {
            println!("    art. {number}: {}", placement.path());
        }
        if placed.len() > 6 {
            println!("    ... and {} more", placed.len() - 6);
        }
    }

    if do_rewrite {
        // The rewrite refuses an empty or drastically shrunken official
        // set instead of writing it out; acting on one has erased a
        // complete law before. A refusal fails the gate and leaves the
        // file exactly as it was.
        let (fixed, sidecar) = match rewrite(&doc, &official, &report) {
            Ok(pair) => pair,
            Err(reason) => {
                println!("  rewrite refused: {reason}");
                return Ok(false);
            }
        };

        // Serialize both documents before writing either. The pair write
        // stages both files and renames the law file last, so neither an
        // interruption nor a failing sidecar write can leave a truncated
        // or half-replaced law file behind.
        let yaml = serde_yaml_ng::to_string(&fixed).map_err(|e| e.to_string())?;
        let sidecar_path = path
            .parent()
            .ok_or("law file has no parent directory")?
            .join(CONTEXT_SIDECAR);
        let sidecar_yaml = serde_yaml_ng::to_string(&sidecar).map_err(|e| e.to_string())?;
        write_atomic_pair((path, &yaml), (&sidecar_path, &sidecar_yaml))?;

        println!(
            "  rewritten: {} article(s) now carry the official text; structure in {}",
            official.len(),
            sidecar_path.display()
        );
        // After a rewrite the file does carry the law, so it may proceed.
        return Ok(true);
    }

    Ok(report.passes())
}

async fn fetch(
    bwb_id: &str,
    valid_from: &str,
    cache: Option<&Path>,
    offline: bool,
) -> Result<String, String> {
    let cached = cache.map(|dir| dir.join(format!("{bwb_id}_{valid_from}.xml")));
    if let Some(p) = &cached
        && let Ok(body) = std::fs::read_to_string(p)
    {
        return Ok(body);
    }
    if offline {
        return Err(format!("no cached toestand for {bwb_id} @ {valid_from}"));
    }

    let url = toestand_url(bwb_id, valid_from);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("regelrecht-law-source")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("{} for {url}", response.status()));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;

    if let Some(p) = &cached {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, &body);
    }
    Ok(body)
}
