//! Run one enrichment against a directory on disk, without a database and
//! without git.
//!
//! `execute_enrich_with_runner` needs a path and a payload, not a job queue
//! and not a checkout, so the real loop can run on a developer machine
//! against a single law. That is the difference between a design that can
//! be tried and one that can only be argued about: every layer of RFC-026
//! is a change to the worker, and this is how those changes get exercised
//! before they touch the pipeline.
//!
//! ```text
//! enrich-once --corpus <root> --law <relative/path.yaml> [--provider claude]
//!             [--model opus] [--effort medium] [--timeout 900]
//!             [--articles 15] [--rounds 2|checks=2,marking=2]
//!             [--article 2.1.i]
//! ```
//!
//! `--corpus` is the directory the law path is relative to, standing in for
//! the checkout the worker would make. Everything the worker writes beside
//! the law (`.enrichment.yaml`, the progress file, the result envelope)
//! lands there, so a second run continues where the first stopped, exactly
//! as it does in production.
//!
//! `--article` names one article and enriches that one, with every entry the
//! harvest hung under it: the run does not walk the document and does not move
//! the cursor, because repairing an article is not progress through the law. A
//! number the law does not have fails the run rather than enriching nothing in
//! silence.
//!
//! `--depth` only goes with `--article`, and it counts **wetssprongen** and not
//! artikelen. Concentric circles, and the circle is the law: staying inside a
//! law is free, crossing to another costs a point, and a law reached straight
//! from the start costs one point however long the route that also finds it.
//! The plan is printed before anything runs and the laws are enriched deepest
//! first, so a producer is translated before the law that reads it. A law that
//! fails does not stop the walk: the run carries on, names every failure at the
//! end and exits non-zero. Stopping on the first one is worst precisely because
//! the order is deepest-first — the first law to fail is the deepest producer,
//! and everything still waiting is shallower, so one failure would cost the
//! whole plan behind it.
//!
//! What that costs is measured and it is not small. From `--article 69` of the
//! Zorgverzekeringswet: depth 0 is 53 articles, depth 1 is 624 across 21 laws,
//! depth 2 is 2 694 across 99, depth 3 is 5 274 across 233.
//! `--max-plan-articles` therefore refuses anything over 200 until it is raised
//! on purpose, which means even depth 1 on this law has to be asked for twice.
//! `--article` repeats, so the seven articles the Wet op de zorgtoeslag reads
//! plan as the one closure they form rather than as seven overlapping ones.
//! See `enrich_v2::closure` for the whole table and the stop rules.
//!
//! `--session-reuse` says whether the calls in this window share one agent
//! session: `window` (the default — every call continues the same session,
//! and each resumed feedback prompt opens by ordering the agent to read the
//! file again), `repair` (only the schema gate continues; the two gates that
//! ask for judgement stay cold) or `off` (every call its own cold process,
//! the behaviour before this existed). The run prints what every call cost, so
//! the two modes can be held against each other on the same law.
//!
//! `--rounds` sets how many feedback rounds a gate may run, either one number
//! for all three or per gate (`schema=1,checks=2`). The run prints what each
//! round did per gate, with the marking count beside the finding count: a
//! round can lower the findings by translating better and by declaring more
//! of the law unmodellable, and the two must not read the same.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use regelrecht_pipeline::enrich::{
    AgentCallRecord, AgentUsage, EnrichConfig, EnrichPayload, FeedbackRounds, LlmProvider,
    ProcessLlmRunner, RunSteps, SessionReuse, execute_enrich_with_runner,
};
use regelrecht_pipeline::enrich_v2::checks;
use regelrecht_pipeline::enrich_v2::closure::{LawIndex, Plan, StopRules, Task, plan_closure};

struct Args {
    corpus: PathBuf,
    law: String,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    timeout: u64,
    articles: usize,
    article: Vec<String>,
    depth: Option<usize>,
    max_plan_articles: usize,
    rounds: FeedbackRounds,
    session_reuse: SessionReuse,
    steps: RunSteps,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut law = None;
    let mut provider = "claude".to_string();
    let mut model = None;
    let mut effort = None;
    let mut timeout = 900u64;
    let mut articles = 15usize;
    let mut article: Vec<String> = Vec::new();
    let mut depth = None;
    let mut max_plan_articles = 200usize;
    let mut rounds = FeedbackRounds::default();
    let mut steps = RunSteps::all();
    let mut session_reuse = SessionReuse::default();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--corpus" => corpus = Some(PathBuf::from(value("--corpus")?)),
            "--law" => law = Some(value("--law")?),
            "--provider" => provider = value("--provider")?,
            "--model" => model = Some(value("--model")?),
            "--effort" => effort = Some(value("--effort")?),
            // Repeatable: the Wet op de zorgtoeslag reads seven articles of
            // the Zorgverzekeringswet, and planning them one at a time would
            // plan seven overlapping closures instead of the one they form.
            "--article" => article.push(value("--article")?),
            "--depth" => {
                depth = Some(
                    value("--depth")?
                        .parse()
                        .map_err(|_| "--depth wants a number of law jumps".to_string())?,
                );
            }
            "--max-plan-articles" => {
                max_plan_articles = value("--max-plan-articles")?
                    .parse()
                    .map_err(|_| "--max-plan-articles wants a number".to_string())?;
            }
            "--rounds" => rounds = FeedbackRounds::parse(&value("--rounds")?)?,
            "--steps" => steps = RunSteps::parse(&value("--steps")?)?,
            "--session-reuse" => session_reuse = SessionReuse::parse(&value("--session-reuse")?)?,
            "--timeout" => {
                timeout = value("--timeout")?
                    .parse()
                    .map_err(|_| "--timeout wants a number of seconds".to_string())?;
            }
            "--articles" => {
                articles = value("--articles")?
                    .parse()
                    .map_err(|_| "--articles wants a number".to_string())?;
            }
            "--help" | "-h" => {
                return Err(
                    "usage: enrich-once --corpus <root> --law <path.yaml> [--provider claude] \
                     [--model opus] [--effort medium] [--timeout 900] [--articles 15] \
                     [--rounds 2|checks=2,marking=2] [--article 69]... [--depth 1] \
                     [--max-plan-articles 200] [--steps window,reconcile] \
                     [--session-reuse window|repair|off]"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }

    Ok(Args {
        corpus: corpus.ok_or("--corpus is required")?,
        law: law.ok_or("--law is required")?,
        provider,
        model,
        effort,
        timeout,
        articles,
        article,
        depth,
        max_plan_articles,
        rounds,
        session_reuse,
        steps,
    })
}

/// Resolve `--depth` into a plan, or `None` when the run is the plain single
/// law it always was.
///
/// Every refusal here happens before an agent is spawned: a depth without an
/// article has no centre to be concentric around, a plan bigger than the caller
/// accepts is a run of days, and both are cheaper to say now than to discover
/// halfway.
fn plan_from_args(args: &Args) -> Result<Option<Plan>, String> {
    let Some(depth) = args.depth else {
        return Ok(None);
    };
    if args.article.is_empty() {
        return Err(
            "--depth needs --article: the depth is a distance from an article of one law, and \
             without that centre there is nothing to be a distance from"
                .to_string(),
        );
    }

    let index = LawIndex::scan(&args.corpus)
        .map_err(|e| format!("cannot read the corpus at {}: {e}", args.corpus.display()))?;
    let plan = plan_closure(
        &args.corpus,
        &args.law,
        &args.article,
        depth,
        &index,
        StopRules::default(),
    )?;

    for line in plan_report(&plan, depth, &args.article) {
        println!("{line}");
    }

    if let Some(refusal) = plan.refuse_above(args.max_plan_articles) {
        return Err(refusal);
    }
    Ok(Some(plan))
}

/// The report a plan prints before the first agent is spawned.
///
/// Built as lines rather than printed straight out because this is the only
/// account of a run that costs hours: it names every law, the totals it will
/// work through, and the edges it decided not to follow. A gap count that
/// silently drops an occurrence misstates what the run is about to do, and
/// nobody re-reads the plan afterwards to catch it.
fn plan_report(plan: &Plan, depth: usize, articles: &[String]) -> Vec<String> {
    let mut lines = vec![format!(
        "=== bouwplan op diepte {depth} vanaf artikel {}",
        articles.join(", ")
    )];
    lines.extend(plan.describe().into_iter().map(|line| format!("  {line}")));
    lines.push(format!(
        "  totaal: {} wetten, {} artikelen, {} entries",
        plan.tasks.len(),
        plan.articles(),
        plan.entries()
    ));
    if !plan.gaps.is_empty() {
        // Per kind and summed over the laws: the same reason for not following
        // an edge occurs at many laws, and what the reader needs is how much of
        // the closure each reason accounts for.
        let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
        for gap in &plan.gaps {
            *by_kind.entry(format!("{:?}", gap.kind)).or_default() += gap.occurrences;
        }
        let summary: Vec<String> = by_kind.iter().map(|(k, v)| format!("{k} {v}")).collect();
        lines.push(format!("  bekende gaten: {}", summary.join(", ")));
    }
    lines
}

/// The payload the local run hands the worker.
///
/// `law_id` is left at its default, which is the empty string: locally there
/// is no database row to name and the worker keys on the path. The two fields
/// that are set have nothing to catch them if they go missing — an absent
/// `yaml_path` enriches nothing at all, and an absent `provider` silently
/// falls back to the worker's environment, so `--provider opencode` would run
/// against Claude and the output would look like an ordinary result.
fn payload_from_args(args: &Args) -> EnrichPayload {
    EnrichPayload {
        yaml_path: args.law.clone(),
        provider: Some(args.provider.clone()),
        ..Default::default()
    }
}

/// The payload for one law of a plan: the same run, pointed at another file.
///
/// The path is the whole difference between the tasks, so dropping it would
/// enrich the law the caller named once per task and report a finished plan.
/// Every other field is the run's own configuration and carries over.
fn task_payload(payload: &EnrichPayload, task: &Task) -> EnrichPayload {
    EnrichPayload {
        yaml_path: task.path.clone(),
        ..payload.clone()
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    // The closure: which laws and which of their articles this order pulls in.
    // Planned first of all, printed whole, and refused when it is bigger than
    // the caller said it would accept. Before the skills check on purpose: a
    // plan that is going to be refused should say so without needing a corpus
    // that is set up to run, and planning writes nothing.
    let plan = match plan_from_args(&args) {
        Ok(plan) => plan,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    // The agent reads its instructions from `.claude/skills/` under the corpus
    // root. Nothing put them there: `ensure_skills` is called from the worker
    // and never from here, so every experiment run so far handed the agent the
    // prompt and nothing else. It wrote `untranslatables` into a v0.7.0 file
    // because that is what it knew, and the schema gate had to repair what an
    // instruction would have prevented.
    //
    // Refusing is the point. A run without skills produces a plausible corpus
    // and an unusable measurement, and the difference is invisible afterwards.
    if let Err(e) = regelrecht_pipeline::enrich::ensure_skills(&args.corpus).await {
        eprintln!(
            "could not place skills under {}: {e}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    if let Err(missing) = skills_present(&args.corpus) {
        eprintln!(
            "the corpus root carries no agent instructions: {missing} is absent.\n\
             Point SKILLS_DIR at a checkout that has .claude/skills, e.g.\n\
             \x20 SKILLS_DIR=/path/to/regelrecht enrich-once --corpus {} ...",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }

    let before = report(&args, "before");

    let provider = match args.provider.as_str() {
        "claude" => LlmProvider::Claude {
            path: "claude".into(),
            model: Some(regelrecht_pipeline::enrich::claude_model(
                args.model.clone(),
                None,
            )),
        },
        "opencode" => LlmProvider::OpenCode {
            path: "opencode".into(),
            model: args.model.clone(),
        },
        other => {
            eprintln!("unknown provider: {other}");
            return ExitCode::from(2);
        }
    };

    let config = EnrichConfig::for_local_run(
        provider,
        Duration::from_secs(args.timeout),
        args.articles,
        args.article.clone(),
        args.rounds,
        args.effort.clone(),
        args.session_reuse,
    );
    let mut config = config;
    config.steps = args.steps;
    let payload = payload_from_args(&args);

    // `source_hash` guards against enriching a law whose text moved under
    // the previous run. Locally there is no base branch to compare with, so
    // an empty hash means "adopt whatever is here", which is what a first
    // run does in production too.
    // Without a subscriber the accounting line the worker emits goes nowhere,
    // and a round can then only be compared on wall clock. Default to info so
    // a plain run reports what it cost; RUST_LOG still overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    // With a plan the run walks it, deepest first; without one it is the
    // single law the caller named, which is every run that existed before
    // `--depth`.
    if let Some(plan) = &plan {
        let mut failures: Vec<String> = Vec::new();
        for (number, task) in plan.tasks.iter().enumerate() {
            println!(
                "\n=== [{}/{}] diepte {} — {} ({} artikelen, {} entries)",
                number + 1,
                plan.tasks.len(),
                task.depth,
                task.law_id,
                task.articles.len(),
                task.entries
            );
            let mut task_config = config.clone();
            task_config.target_articles = task.articles.clone();
            let task_payload = task_payload(&payload, task);
            match execute_enrich_with_runner(
                &task_payload,
                &args.corpus,
                &task_config,
                "",
                &ProcessLlmRunner,
            )
            .await
            {
                Ok((result, changed)) => {
                    println!(
                        "  {} of {} articles carry machine_readable, files touched {}",
                        result.articles_with_machine_readable,
                        result.articles_total,
                        changed.len()
                    );
                    print_feedback(&result.feedback);
                }
                Err(e) => {
                    // One law out of many. Stopping the whole plan on it would
                    // throw away every law already translated, and the laws
                    // after it do not depend on this one being finished — they
                    // are shallower, so they read it rather than feed it.
                    // Loud, and the run reports a failure at the end.
                    //
                    // The code used to return here, which is the opposite of
                    // what the comment promises and worst exactly where the
                    // plan is deepest-first: the first producer that fails
                    // aborts before any shallower law has been touched, so one
                    // failure costs every law behind it.
                    eprintln!("  enrichment failed for {}: {e}", task.law_id);
                    failures.push(format!("{}: {e}", task.law_id));
                }
            }
        }
        println!("\n=== plan afgelopen: {} wetten", plan.tasks.len());
        for line in plan_failure_report(&failures, plan.tasks.len()) {
            eprintln!("{line}");
        }
        return plan_verdict(&failures);
    }

    let outcome =
        execute_enrich_with_runner(&payload, &args.corpus, &config, "", &ProcessLlmRunner).await;

    match outcome {
        Ok((result, changed)) => {
            println!("\n=== enrichment finished");
            println!(
                "  {} of {} articles carry machine_readable, coverage {:.2}",
                result.articles_with_machine_readable, result.articles_total, result.coverage_score
            );
            if !result.untranslatables.is_empty() {
                println!("  untranslatables: {}", result.untranslatables.len());
            }
            println!("  files touched: {}", changed.len());
            print_feedback(&result.feedback);
            print_cost(&result.session_reuse, &result.agent_calls, result.usage);
        }
        Err(e) => {
            println!("\n=== enrichment failed: {e}");
            report(&args, "after");
            return ExitCode::FAILURE;
        }
    }

    let after = report(&args, "after");
    println!("\n=== deterministic checks");
    let shown = |counts: Option<(usize, usize)>, pick: fn((usize, usize)) -> usize| {
        counts.map_or_else(|| "unreadable".to_string(), |c| pick(c).to_string())
    };
    println!(
        "  schema errors {} → {}",
        shown(before, |c| c.0),
        shown(after, |c| c.0)
    );
    println!(
        "  findings      {} → {}",
        shown(before, |c| c.1),
        shown(after, |c| c.1)
    );

    verdict(after.map(|c| c.0))
}

/// What a walked plan reports when laws failed, and nothing when none did.
///
/// At the end and by name, because the walk runs on. A run that translated
/// nine laws and failed on the tenth has done nine laws' worth of work, and
/// the line that says which one failed has scrolled far past by then.
fn plan_failure_report(failures: &[String], laws: usize) -> Vec<String> {
    if failures.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("\n=== {} of {laws} laws failed:", failures.len())];
    lines.extend(failures.iter().map(|failure| format!("  {failure}")));
    lines
}

/// The exit code of a walked plan: any law that failed fails the run.
///
/// Separate from the walk so it can be held to the rule the walk's own comment
/// states. The walk carries on past a failure, and the exit code is the only
/// thing left that still says one happened.
fn plan_verdict(failures: &[String]) -> ExitCode {
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The verdict of a finished run: the schema errors that are left over.
///
/// Read from a script and from a loop over a list of laws, so it has to be
/// the answer to "is this corpus in a state I can use". Findings are not in
/// it — those are the work the run is for — but a law the schema refuses is
/// a law the engine will not load, and a run that ends on one has not
/// finished.
fn verdict(schema_errors_after: Option<usize>) -> ExitCode {
    match schema_errors_after {
        Some(0) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// Print what every feedback round did, per gate.
///
/// One line per round, and each line carries both numbers: the findings the
/// round took away and the markings the file gained while doing it. A round
/// that halves the findings by declaring half the law unmodellable looks
/// identical to a good one on the finding count alone, which is the exact
/// confound this measurement has to survive.
fn print_feedback(feedback: &[regelrecht_pipeline::enrich::GateFeedback]) {
    if feedback.is_empty() {
        return;
    }
    println!("\n=== feedback rounds (findings | markings)");
    for gate in feedback {
        if gate.rounds.is_empty() {
            println!("  {:<8} no findings, no round run", gate.gate);
            continue;
        }
        println!(
            "  {:<8} {} → {} findings over {} round(s)",
            gate.gate,
            gate.findings_initial,
            gate.findings_final,
            gate.rounds.len()
        );
        for round in &gate.rounds {
            let markings = |n: Option<usize>| n.map_or("?".to_string(), |n| n.to_string());
            println!(
                "    round {}: findings {} → {} ({:+}) | markings {} → {} | file {} | {}",
                round.round,
                round.findings_before,
                round.findings_after,
                round.findings_after as i64 - round.findings_before as i64,
                markings(round.markings_before),
                markings(round.markings_after),
                if round.file_changed {
                    "changed"
                } else {
                    "unchanged"
                },
                match round.stopped {
                    None => "continues".to_string(),
                    Some(stop) => format!("stopped: {stop:?}"),
                }
            );
        }
    }
}

/// Run the deterministic checks and print a one-line summary. Returns
/// `(schema errors, findings)` so the two runs can be compared, and `None`
/// when there was nothing to check.
///
/// `None` rather than a pair of zeros, and that is the whole reason this
/// returns an `Option`. A file that cannot be read produces no schema errors
/// for the same reason it produces no findings — nobody looked — and
/// [`verdict`] used to read that as a clean law and hand the caller a success.
/// That is the exit code a loop over a list of laws steers on.
fn report(args: &Args, when: &str) -> Option<(usize, usize)> {
    let path = args.corpus.join(&args.law);
    let Ok(yaml) = std::fs::read_to_string(&path) else {
        println!("=== {when}: cannot read {}", path.display());
        return None;
    };
    let report = checks::run(&yaml, Some(&args.corpus));
    let counts = report.by_check();
    let summary: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!(
        "=== {when}: schema {}, {}",
        if report.schema.is_empty() {
            "valid".to_string()
        } else {
            format!("{} error(s)", report.schema.len())
        },
        if summary.is_empty() {
            "nothing to report".to_string()
        } else {
            summary.join(" ")
        }
    );
    for e in report.schema.iter().take(5) {
        println!("    schema: {e}");
    }
    Some((report.schema.len(), report.findings.len()))
}

/// Print what the window cost: one line per call, then the total.
///
/// Per call and not only per window, because the question this setting exists
/// to answer is which call got cheaper. A resumed round beside a cold one, on
/// the same law and in the same window, is the only comparison that settles
/// whether continuing the session paid for itself.
fn print_cost(mode: &str, calls: &[AgentCallRecord], total: Option<AgentUsage>) {
    if calls.is_empty() {
        return;
    }
    println!("\n=== cost (session reuse: {mode})");
    println!(
        "  {:<10} {:>8} {:>9} {:>9} {:>12} {:>12} {:>9}",
        "step", "session", "input", "output", "cache read", "cache write", "cost"
    );
    for call in calls {
        let (input, output, cache, write, cost) = call.usage.map_or((0, 0, 0, 0, 0), |u| {
            (
                u.input_tokens,
                u.output_tokens,
                u.cache_read_tokens,
                u.cache_write_tokens,
                u.cost_millicents,
            )
        });
        println!(
            "  {:<10} {:>8} {:>9} {:>9} {:>12} {:>12} {:>9}",
            call.step,
            if call.resumed { "resumed" } else { "cold" },
            input,
            output,
            cache,
            write,
            money(cost)
        );
    }
    if let Some(u) = total {
        println!(
            "  {:<10} {:>8} {:>9} {:>9} {:>12} {:>12} {:>9}",
            "window",
            calls.len(),
            u.input_tokens,
            u.output_tokens,
            u.cache_read_tokens,
            u.cache_write_tokens,
            money(u.cost_millicents)
        );
    }
}

/// Thousandths of a cent as dollars, because that is the unit the usage
/// record carries and an integer is the only honest way to hold money.
fn money(millicents: u64) -> String {
    format!("${}.{:04}", millicents / 100_000, millicents % 100_000 / 10)
}

/// Whether the corpus root carries the instructions the agent needs.
///
/// Checked by path rather than by content: the prompt names these files and
/// the agent opens them itself, so their absence is silent on both sides.
fn skills_present(corpus: &Path) -> Result<(), String> {
    for required in [
        ".claude/skills/law-generate/SKILL.md",
        ".claude/skills/law-generate/reference.md",
    ] {
        if !corpus.join(required).exists() {
            return Err(required.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use regelrecht_pipeline::enrich_v2::closure::{Gap, GapKind};

    /// Three laws, the same shape the closure tests use: A reads B and C
    /// straight, B reads C. The paths are what `--law` and the plan's tasks
    /// carry, so the fixture has to be a real directory and not a stub.
    fn corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "regulation/nl/wet/wet_a/2026-01-01.yaml",
            r"$id: wet_a
regulatory_layer: WET
bwb_id: BWBR0000001
articles:
  - number: '1'
    text: De hoogte volgt uit artikel 5 van wet B en uit artikel 9 van wet C.
    references:
      - id: ref1
        bwb_id: BWBR0000002
        artikel: '5'
      - id: ref2
        bwb_id: BWBR0000003
        artikel: '9'
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_b/2026-01-01.yaml",
            r"$id: wet_b
regulatory_layer: WET
bwb_id: BWBR0000002
articles:
  - number: '5'
    text: Het bedrag wordt berekend met artikel 6.
",
        );
        write(
            &dir,
            "regulation/nl/wet/wet_c/2026-01-01.yaml",
            r"$id: wet_c
regulatory_layer: WET
bwb_id: BWBR0000003
articles:
  - number: '9'
    text: Het percentage is tien procent.
",
        );
        dir
    }

    fn write(dir: &tempfile::TempDir, rel: &str, body: &str) {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn args(corpus: &tempfile::TempDir) -> Args {
        Args {
            corpus: corpus.path().to_path_buf(),
            law: "regulation/nl/wet/wet_a/2026-01-01.yaml".to_string(),
            provider: "claude".to_string(),
            model: None,
            effort: None,
            timeout: 900,
            articles: 15,
            article: Vec::new(),
            depth: None,
            max_plan_articles: 200,
            rounds: FeedbackRounds::default(),
            session_reuse: SessionReuse::default(),
            steps: RunSteps::all(),
        }
    }

    /// Without `--depth` the run is the single law it always was, and planning
    /// must not invent a closure around it.
    #[test]
    fn test_without_depth_there_is_no_plan() {
        let dir = corpus();
        assert_eq!(plan_from_args(&args(&dir)), Ok(None));
    }

    /// With a depth the plan is the closure, and it has to arrive whole: the
    /// laws it found, deepest first, so a producer is translated before the law
    /// that reads it. A run that receives no plan quietly enriches one law and
    /// reports success for a job it did not do.
    #[test]
    fn test_a_depth_plans_the_laws_it_reaches() {
        let dir = corpus();
        let mut args = args(&dir);
        args.depth = Some(1);
        args.article = vec!["1".to_string()];

        let plan = plan_from_args(&args)
            .expect("the corpus is readable")
            .expect("a depth resolves to a plan");

        let law_ids: Vec<&str> = plan.tasks.iter().map(|t| t.law_id.as_str()).collect();
        assert!(
            law_ids.contains(&"wet_a") && law_ids.contains(&"wet_b") && law_ids.contains(&"wet_c"),
            "the plan must carry every law the depth reaches: {law_ids:?}"
        );
        assert!(
            plan.tasks.first().map(|t| t.depth) >= plan.tasks.last().map(|t| t.depth),
            "deepest first, so a producer comes before its reader"
        );
        assert!(
            plan.articles() > 0,
            "a plan with no articles enriches nothing"
        );
    }

    /// A depth is a distance from an article. Without one there is no centre,
    /// and the refusal happens before an agent is spawned.
    #[test]
    fn test_a_depth_without_an_article_is_refused() {
        let dir = corpus();
        let mut args = args(&dir);
        args.depth = Some(1);

        let err = plan_from_args(&args).unwrap_err();
        assert!(
            err.contains("--article"),
            "the refusal must name what is missing: {err}"
        );
    }

    /// The limit is a refusal and not a warning: a plan over it stops the run
    /// rather than starting days of work nobody asked for.
    #[test]
    fn test_a_plan_over_the_limit_is_refused() {
        let dir = corpus();
        let mut args = args(&dir);
        args.depth = Some(1);
        args.article = vec!["1".to_string()];
        args.max_plan_articles = 0;

        let err = plan_from_args(&args).unwrap_err();
        assert!(
            err.contains("--depth") || err.contains("limit"),
            "the refusal must say what to lower: {err}"
        );
    }

    fn task(law_id: &str, articles: usize) -> Task {
        Task {
            depth: 0,
            bwb_id: "BWBR0000001".to_string(),
            law_id: law_id.to_string(),
            path: format!("regulation/nl/wet/{law_id}/2026-01-01.yaml"),
            articles: (0..articles).map(|n| n.to_string()).collect(),
            entries: articles * 2,
        }
    }

    /// A line per thing the plan actually has. An empty list is not a finding,
    /// and printing "bekende gaten:" with nothing behind it reads as the
    /// opposite of what it means.
    #[test]
    fn test_the_report_leaves_out_what_the_plan_does_not_have() {
        let plan = Plan {
            tasks: vec![task("wet_a", 2)],
            gaps: Vec::new(),
        };
        let report = plan_report(&plan, 1, &["1".to_string()]).join("\n");

        assert!(report.contains("bouwplan op diepte 1 vanaf artikel 1"));
        assert!(report.contains("totaal: 1 wetten, 2 artikelen, 4 entries"));
        assert!(
            !report.contains("bekende gaten"),
            "no edge was left unfollowed, so the line must not appear: {report}"
        );
    }

    /// And the other way round: what the plan does have is reported, with the
    /// occurrences summed per reason.
    ///
    /// Summed, because the same reason occurs at several laws and the reader
    /// needs the share of the closure it accounts for. A count that is one
    /// law's occurrences instead of the total makes an unharvested corpus look
    /// like an incident.
    #[test]
    fn test_the_gap_summary_sums_the_occurrences_per_reason() {
        let plan = Plan {
            tasks: vec![task("wet_a", 2)],
            gaps: vec![
                Gap {
                    kind: GapKind::OutsideCorpus,
                    bwb_id: "BWBR0000004".to_string(),
                    occurrences: 3,
                },
                Gap {
                    kind: GapKind::OutsideCorpus,
                    bwb_id: "BWBR0000005".to_string(),
                    occurrences: 4,
                },
                Gap {
                    kind: GapKind::Delegated,
                    bwb_id: "BWBR0000006".to_string(),
                    occurrences: 2,
                },
            ],
        };
        let report = plan_report(&plan, 2, &["1".to_string(), "2".to_string()]).join("\n");

        assert!(report.contains("vanaf artikel 1, 2"));
        assert!(
            report.contains("bekende gaten: Delegated 2, OutsideCorpus 7"),
            "seven edges point outside the corpus, not four and not three: {report}"
        );
    }

    /// The counts the run compares before and after, and the first of them is
    /// what the process exits on. A law the schema refuses has to count for
    /// more than a law it accepts: a report that answers the same either way
    /// lets the run exit successfully on a corpus it just broke.
    ///
    /// Compared rather than pinned to a number, because the number belongs to
    /// the checks and may move; what may not move is that the broken law
    /// scores worse.
    #[test]
    fn test_the_report_counts_a_refused_law_worse_than_an_accepted_one() {
        let dir = corpus();
        let mut args = args(&dir);
        let accepted = report(&args, "before");

        // `articles` as a mapping where the schema wants a list.
        write(
            &dir,
            "regulation/nl/wet/wet_stuk/2026-01-01.yaml",
            r"$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.7.0/schema/v0.7.0/schema.json
$id: wet_stuk
regulatory_layer: WET
bwb_id: BWBR0000009
articles:
  number: '1'
",
        );
        args.law = "regulation/nl/wet/wet_stuk/2026-01-01.yaml".to_string();
        let refused = report(&args, "before");

        let (accepted, refused) = (accepted.unwrap(), refused.unwrap());
        assert!(
            refused.0 > accepted.0,
            "a law the schema refuses must count more errors than one it accepts: \
             {refused:?} against {accepted:?}"
        );

        // A file that is not there is not a clean law: there are no counts at
        // all, and the run says so on the line above them.
        args.law = "regulation/nl/wet/bestaat_niet/2026-01-01.yaml".to_string();
        assert_eq!(report(&args, "before"), None);
    }

    /// A corpus without agent instructions produces a plausible law and an
    /// unusable measurement, and afterwards the two are indistinguishable. The
    /// check is by path, so it has to answer both ways: present is not a
    /// finding, and absent names the file that is missing.
    #[test]
    fn test_skills_are_looked_for_by_path_and_reported_by_name() {
        let dir = corpus();
        let missing = skills_present(dir.path()).unwrap_err();
        assert!(
            missing.contains("law-generate"),
            "the refusal must name the file it did not find: {missing}"
        );

        write(&dir, ".claude/skills/law-generate/SKILL.md", "# skill\n");
        assert!(
            skills_present(dir.path()).is_err(),
            "one of the two files is not the pair the agent reads"
        );

        write(
            &dir,
            ".claude/skills/law-generate/reference.md",
            "# reference\n",
        );
        assert_eq!(skills_present(dir.path()), Ok(()));
    }

    /// A plan walks on past a law that failed, and says so at the end.
    ///
    /// The walk is deepest-first, so the first law to fail is the deepest
    /// producer and everything after it is shallower. Returning on that first
    /// failure — which is what the code did, against what its own comment
    /// promised — threw away every law behind it, including laws that do not
    /// depend on the failed one at all.
    #[test]
    fn test_a_failed_law_does_not_take_the_rest_of_the_plan_with_it() {
        let failures = vec!["wet_d: de agent gaf niets terug".to_string()];
        let report = plan_failure_report(&failures, 4).join("\n");

        assert!(
            report.contains("1 of 4 laws failed"),
            "the run names how much of the plan failed: {report}"
        );
        assert!(
            report.contains("wet_d: de agent gaf niets terug"),
            "and which law it was, with the reason: {report}"
        );
        assert_eq!(
            plan_verdict(&failures),
            ExitCode::FAILURE,
            "carrying on is not the same as succeeding"
        );
    }

    /// And a plan in which nothing failed says nothing and exits clean. A
    /// failure report over an empty list reads as a failure that happened.
    #[test]
    fn test_a_plan_without_failures_reports_none() {
        assert!(plan_failure_report(&[], 4).is_empty());
        assert_eq!(plan_verdict(&[]), ExitCode::SUCCESS);
    }

    /// One schema error left is a failed run, and none is a finished one.
    /// Anything looser makes the exit code say the same thing either way.
    #[test]
    fn test_the_verdict_turns_on_a_single_leftover_schema_error() {
        assert_eq!(verdict(Some(0)), ExitCode::SUCCESS);
        assert_eq!(verdict(Some(1)), ExitCode::FAILURE);
        assert_eq!(verdict(Some(7)), ExitCode::FAILURE);
    }

    /// A law nobody could read is not a law with nothing wrong with it.
    ///
    /// The unreadable case produced no schema errors for the same reason it
    /// produced no findings — nobody looked — and the verdict read that as a
    /// clean corpus. This is the exit code a loop over a list of laws steers
    /// on, so the silence came out as a success.
    #[test]
    fn test_an_unreadable_law_is_not_a_clean_one() {
        let dir = corpus();
        let mut args = args(&dir);
        assert!(
            report(&args, "before").is_some(),
            "de wet die er staat wordt wel gemeten"
        );

        args.law = "regulation/nl/wet/bestaat_niet/2026-01-01.yaml".to_string();
        assert_eq!(report(&args, "before"), None);
        assert_eq!(
            verdict(report(&args, "before").map(|c| c.0)),
            ExitCode::FAILURE,
            "een onleesbare wet mag geen geslaagde run opleveren"
        );
    }

    /// The provider reports money in thousandths of a cent, and an integer is the
    /// only honest way to carry it. The formatting is where it can still go
    /// wrong, and a cost line that is off by a factor is worse than no line.
    #[test]
    fn test_money_reads_thousandths_of_a_cent_as_dollars() {
        assert_eq!(money(0), "$0.0000");
        assert_eq!(money(123_456), "$1.2345");
        assert_eq!(money(100_000), "$1.0000");
    }

    /// Every task of a plan enriches its own law. The path is the only thing
    /// that differs between them, so a task payload that keeps the caller's
    /// path would enrich that one law once per task and report a plan it never
    /// walked.
    #[test]
    fn test_a_task_payload_points_at_the_law_of_that_task() {
        let dir = corpus();
        let base = payload_from_args(&args(&dir));
        let task = task("wet_c", 1);

        let payload = task_payload(&base, &task);
        assert_eq!(payload.yaml_path, task.path);
        assert_ne!(payload.yaml_path, base.yaml_path);
        assert_eq!(
            payload.provider, base.provider,
            "the rest of the run's configuration carries over"
        );
    }

    /// The path and the provider have to reach the worker. `yaml_path` is the
    /// law that gets enriched, and an absent `provider` falls back to the
    /// worker's environment, so `--provider opencode` would run against the
    /// other model and produce a result that looks ordinary.
    #[test]
    fn test_the_payload_names_the_law_and_the_provider() {
        let dir = corpus();
        let mut args = args(&dir);
        args.provider = "opencode".to_string();

        let payload = payload_from_args(&args);
        assert_eq!(payload.yaml_path, "regulation/nl/wet/wet_a/2026-01-01.yaml");
        assert_eq!(payload.provider.as_deref(), Some("opencode"));
    }
}
