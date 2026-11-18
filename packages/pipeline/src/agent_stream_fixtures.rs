//! Recorded agent output, run through the supervisor by a fake CLI that prints
//! a fixture from `tests/fixtures/agent-streams/`.
//!
//! These pin what one run reports per provider (usage, model, error text and
//! the activity summary) as the code stands, so that moving the provider
//! dialects out of `enrich.rs` can be shown to change none of it. The expected
//! values are read off the fixture lines by hand, not computed by the code
//! under test. `README.md` next to the fixtures says where each one came from.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::enrich::{
    AgentRun, AgentUsage, EnrichConfig, LlmProvider, SessionAction, ToolPolicy, run_llm_subprocess,
};
use crate::error::Result;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/agent-streams")
        .join(name)
}

/// A fake provider CLI that prints `name` without its `#` header lines and
/// exits with `code`. The script is named after the provider, as the real
/// binary would be.
#[allow(clippy::unwrap_used)] // test setup; a failure here is the failure
fn fake_cli(binary: &str, name: &str, code: i32) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(binary);
    let body = format!(
        "#!/bin/sh\nsed '/^#/d' '{}'\nexit {code}\n",
        fixture(name).display()
    );
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, path)
}

fn claude(name: &str, code: i32) -> (tempfile::TempDir, LlmProvider) {
    let (dir, path) = fake_cli("claude", name, code);
    (dir, LlmProvider::Claude { path, model: None })
}

fn opencode(name: &str, code: i32) -> (tempfile::TempDir, LlmProvider) {
    let (dir, path) = fake_cli("opencode", name, code);
    let model = Some("vlam/kimi-k3-prepaid".to_string());
    (dir, LlmProvider::OpenCode { path, model })
}

async fn run(provider: &LlmProvider, cwd: &Path) -> (Result<AgentRun>, EnrichConfig) {
    let mut config = EnrichConfig::for_test(provider.clone());
    config.timeout = Duration::from_secs(20);
    config.idle_timeout = None;
    config.activity.begin_call("translate");
    // The fake CLI was just written; a test forking beside this one can hold
    // it open for a moment, and exec then fails with ETXTBSY. That is the test
    // writing an executable, not the code under test.
    let mut attempt = 0;
    let out = loop {
        let out = run_llm_subprocess(
            provider,
            "prompt",
            None,
            cwd,
            &config,
            ToolPolicy {
                allow_bash: false,
                deny: &[],
            },
            SessionAction::Cold,
        )
        .await;
        let busy = matches!(&out, Err(e) if e.to_string().contains("Text file busy"));
        attempt += 1;
        if !busy || attempt == 10 {
            break out;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    (out, config)
}

#[tokio::test]
async fn a_claude_run_reports_the_model_and_the_usage_of_its_result_event() {
    let (dir, provider) = claude("claude-success.jsonl", 0);
    let (out, config) = run(&provider, dir.path()).await;
    let run = out.expect("the captured run succeeds");

    // From the `init` event.
    assert_eq!(run.model.as_deref(), Some("claude-haiku-5-5"));
    // From the closing `result` event, not from the per-message usage in the
    // stream events before it. total_cost_usd 0.00302066 is 302 millicents.
    assert_eq!(
        run.usage,
        Some(AgentUsage {
            input_tokens: 4,
            output_tokens: 236,
            cache_read_tokens: 35_646,
            cache_write_tokens: 12_729,
            cost_millicents: 302,
        })
    );

    // Three assistant events over two message ids are two turns; the one
    // tool call is the Read.
    let s = config.activity.snapshot();
    assert_eq!(s["model"], "claude-haiku-5-5");
    assert_eq!(s["turns"], 2);
    assert_eq!(s["tool_calls"], 1);
    assert_eq!(s["last_tool"], "Read /work/law.yaml");
}

#[tokio::test]
async fn a_failed_claude_run_says_what_its_result_event_reported() {
    // Exit 1 and nothing on stderr: the reason is only in the result event.
    let (dir, provider) = claude("claude-error.jsonl", 1);
    let (out, _) = run(&provider, dir.path()).await;
    let err = out.expect_err("the captured run fails").to_string();
    assert!(
        err.contains(
            "claude exited with exit status: 1: There's an issue with the selected model \
             (claude-does-not-exist-1). It may not exist or you may not have access to it. \
             Run --model to pick a different model.; agent activity: 1 turns, 0 tool calls"
        ),
        "{err}"
    );
}

#[tokio::test]
async fn an_opencode_run_sums_its_steps_and_keeps_the_configured_model() {
    let (dir, provider) = opencode("opencode-success.jsonl", 0);
    let (out, config) = run(&provider, dir.path()).await;
    let run = out.expect("the run succeeds");

    // opencode names no model in its output: the configured one is recorded.
    assert_eq!(run.model.as_deref(), Some("vlam/kimi-k3-prepaid"));
    // The sum of the two step_finish events. Reasoning counts as output:
    // 900 + 5100 + 300 + 0. Cost 0.0123 + 0.002 dollars.
    assert_eq!(
        run.usage,
        Some(AgentUsage {
            input_tokens: 81_000,
            output_tokens: 6_300,
            cache_read_tokens: 69_000,
            cache_write_tokens: 10,
            cost_millicents: 1_430,
        })
    );

    let s = config.activity.snapshot();
    assert_eq!(s["model"], "vlam/kimi-k3-prepaid");
    assert_eq!(s["turns"], 2);
    assert_eq!(s["tool_calls"], 2);
    assert_eq!(s["last_tool"], "edit law.yaml");
}

#[tokio::test]
async fn an_opencode_step_cut_off_by_the_output_limit_shows_in_the_summary() {
    let (dir, provider) = opencode("opencode-cut-off.jsonl", 0);
    let (out, config) = run(&provider, dir.path()).await;
    // The process exits 0, so the run itself succeeds; only the activity
    // summary carries the cut-off.
    let run = out.expect("the run exits 0");
    assert_eq!(
        run.usage.map(|u| u.output_tokens),
        Some(44_000),
        "32000 output + 12000 reasoning"
    );
    let summary = config.activity.summary();
    assert!(
        summary.contains("1 step(s) cut off by the output limit"),
        "{summary}"
    );
}

#[tokio::test]
async fn a_failed_opencode_run_reports_the_exit_but_not_the_error_event() {
    let (dir, provider) = opencode("opencode-error.jsonl", 1);
    let (out, _) = run(&provider, dir.path()).await;
    let err = out.expect_err("the run fails").to_string();
    assert!(
        err.contains("opencode exited with exit status: 1; agent activity: 1 turns, 0 tool calls"),
        "{err}"
    );
    // As the code stands, the error event's reason is logged but does not
    // reach the job's error message, unlike claude's result event. Pinned
    // here so that changing it is a decision, not a side effect.
    assert!(!err.contains("401"), "{err}");
}

/// Everything the `tracing` macros write while it is the default subscriber.
#[derive(Clone, Default)]
struct CapturedLog(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut log) = self.0.lock() {
            log.extend_from_slice(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLog {
    type Writer = CapturedLog;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn a_cut_off_step_logs_what_it_wrote_as_reasoning_and_text() {
    // The sizes travel from the dialect through the supervisor, which counts
    // them per call, to the step's log line. A single-threaded runtime keeps
    // the drain task on this thread, under this subscriber.
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(log.clone())
        .with_ansi(false)
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let (dir, provider) = opencode("opencode-cut-off.jsonl", 0);
    let (out, _) = run(&provider, dir.path()).await;
    out.expect("the run exits 0");

    let text = log
        .0
        .lock()
        .map(|l| String::from_utf8_lossy(&l).into_owned())
        .unwrap_or_default();
    let line = text
        .lines()
        .find(|l| l.contains("agent step cut off by the output limit"))
        .unwrap_or_else(|| panic!("no cut-off line in:\n{text}"));
    let reasoning = "Eerst alle artikelen doorlopen.".chars().count();
    let written = "Ik begin met artikel 1.".chars().count();
    assert!(
        line.contains(&format!("reasoning_chars={reasoning}")),
        "{line}"
    );
    assert!(line.contains(&format!("text_chars={written}")), "{line}");
    assert!(line.contains("tool_input_chars=0"), "{line}");
}
