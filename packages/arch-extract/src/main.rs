//! `arch-extract` — generates the code-derived architecture model for the
//! regelrecht workspace, and (via `serve`) hosts the local explorer that
//! renders it.
//!
//! ```text
//! arch-extract [generate] [--out <path>] [--stdout] [--deep a,b | --deep-all] [--manifest-path <p>]
//! arch-extract serve [--port <n>] [--manifest-path <p>] [--ui-dir <dir>]
//! ```
//!
//! `generate` (the default command) writes the `model.json` that the local
//! architecture explorer renders. The model is **generated on-demand and never
//! committed** — it is regenerated from the working tree, so it is always
//! current by construction. `--stdout` prints the model instead of writing it,
//! handy for a quick inspection.
//!
//! `serve` runs the explorer: a small Axum server that regenerates the model
//! on-demand at `GET /api/model` (cached on source mtime) and serves the built
//! UI at `/`. See `serve.rs` and `README.md`.
//!
//! Run it from `packages/` (as `just arch-generate`/`just arch-explore` do) so
//! `cargo metadata` discovers the workspace; the repo root is derived from
//! there and the model defaults to `docs/src/content/architecture/model.json`
//! (a gitignored path).

mod build;
mod crate_graph;
mod js_pass;
mod model;
mod prose;
mod serve;
mod syn_pass;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use build::{DeepScope, build_model};

/// Default, repo-relative location the generated model is written to. This path
/// is gitignored: the model is a local, on-demand artifact, not committed.
const DEFAULT_OUT: &str = "docs/src/content/architecture/model.json";

struct Args {
    out: Option<PathBuf>,
    stdout: bool,
    manifest_path: Option<PathBuf>,
    deep: DeepScope,
}

fn parse_args<I: Iterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut out = None;
    let mut stdout = false;
    let mut manifest_path = None;
    let mut deep = DeepScope::All;

    let mut it = args;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "generate" => {}
            "--stdout" => stdout = true,
            "--deep-all" => deep = DeepScope::All,
            "--deep" => {
                let list = it.next().ok_or("--deep needs a comma-separated value")?;
                deep = DeepScope::Only(list.split(',').map(|s| s.trim().to_string()).collect());
            }
            "--out" => {
                out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?));
            }
            "--manifest-path" => {
                manifest_path = Some(PathBuf::from(
                    it.next().ok_or("--manifest-path needs a value")?,
                ));
            }
            "-h" | "--help" => {
                println!(
                    "arch-extract [generate] [--out <path>] [--stdout] [--deep a,b | --deep-all] [--manifest-path <p>]\n\
                     arch-extract serve [--port <n>] [--manifest-path <p>] [--ui-dir <dir>]\n\
                     arch-extract prose <status|check|sync|bless> [--json] [--prose-dir <d>] [--manifest-path <p>]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }

    Ok(Args {
        out,
        stdout,
        manifest_path,
        deep,
    })
}

fn resolve_out(out: Option<PathBuf>, repo_root: &Path) -> PathBuf {
    out.unwrap_or_else(|| repo_root.join(DEFAULT_OUT))
}

fn run_generate<I: Iterator<Item = String>>(
    args: I,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args = parse_args(args)?;
    let (mvalue, repo_root) = build_model(args.manifest_path.as_deref(), &args.deep)?;
    let json = mvalue.to_json()?;

    if args.stdout {
        print!("{json}");
        return Ok(ExitCode::SUCCESS);
    }

    let out = resolve_out(args.out, &repo_root);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out, &json)?;
    eprintln!(
        "arch-extract: wrote {} node(s), {} edge(s) → {}",
        mvalue.nodes.len(),
        mvalue.edges.len(),
        out.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // `serve` and `prose` are dispatched to their own handlers; everything else
    // is `generate` (the default, with an optional leading `generate` token).
    if args.first().map(String::as_str) == Some("serve") {
        args.remove(0);
        return serve::run(&args);
    }
    if args.first().map(String::as_str) == Some("prose") {
        args.remove(0);
        return prose::run(&args);
    }

    match run_generate(args.into_iter()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("arch-extract: error: {e}");
            ExitCode::FAILURE
        }
    }
}
