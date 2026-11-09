//! What the agent subprocess is doing while it runs.
//!
//! The claude provider runs with `--output-format stream-json`, which prints
//! one JSON event per line as the session goes: the init, every assistant
//! message, every tool result, partial message chunks, and the closing
//! `result` with the usage. Before this the worker ran it with
//! `--output-format json`, which prints one object when the process exits, so
//! a run that hit its timeout left nothing behind to say what it had been
//! doing for twenty minutes.
//!
//! [`ActivityHandle`] is shared between the subprocess drain (which records
//! events), the worker's progress poller (which writes a snapshot to
//! `jobs.progress`), and the stall watchdog (which stops a run that has gone
//! silent). Any provider counts as active while it writes to stdout; what its
//! lines mean is read by the provider's dialect (`crate::agent`) and arrives
//! here as [`AgentEvent`]s.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use crate::agent::{AgentEvent, StepFinish};

/// How many recent events a snapshot and an error message carry.
const RECENT_EVENTS: usize = 8;

/// The longest event line that is parsed. Longer lines (a tool result that
/// inlines a whole file) still count as activity but are not read, so one
/// large event cannot make the worker hold megabytes.
pub const MAX_EVENT_LINE: usize = 1024 * 1024;

/// Starts the activity summary that ends an agent error message. The worker
/// classifies errors by substring; [`without_activity`] cuts the summary off
/// first, so a tool target in it can never decide how a failure is treated.
pub const ACTIVITY_MARKER: &str = "; agent activity:";

/// The error message without the activity summary [`ActivityHandle::summary`]
/// appended to it, for matching on substrings.
pub fn without_activity(err: &str) -> &str {
    err.find(ACTIVITY_MARKER).map_or(err, |i| &err[..i])
}

/// What the agent has done in the current job, across all its calls.
#[derive(Debug, Default)]
struct Activity {
    /// Label of the call in progress (`translate`, a gate name, ...).
    call: Option<String>,
    /// Calls started in this job.
    calls: u32,
    call_started: Option<Instant>,
    /// Agent subprocesses running now. More than one when windows run side
    /// by side; the clock is cleared only when the last one ends.
    in_flight: u32,
    last_output: Option<Instant>,
    /// Assistant messages seen, counted by message id.
    turns: u32,
    last_message_id: Option<String>,
    tool_calls: u32,
    last_tool: Option<String>,
    /// The model the latest agent call named in its `init` event. Kept after
    /// the call ends, so a stopped job still shows what it ran on; with
    /// windows side by side the last one to start wins.
    model: Option<String>,
    recent: VecDeque<String>,
    /// opencode model steps that ended on `length`: cut off by the output
    /// limit rather than finished. A count, not "the last step", because the
    /// calls of one job (windows side by side, gate rounds) share this state.
    steps_cut_off: u32,
}

/// Shared view of the agent's activity. Cheap to clone; every clone sees the
/// same state.
#[derive(Debug, Clone, Default)]
pub struct ActivityHandle(Arc<Mutex<Activity>>);

impl ActivityHandle {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<R>(&self, f: impl FnOnce(&mut Activity) -> R) -> R {
        // A poisoned lock only means a panic elsewhere mid-update; the
        // counters are still good enough to report.
        let mut guard = match self.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }

    /// Mark the start of an agent call. `label` names it in snapshots.
    /// Name the agent call about to start. With windows side by side the
    /// last one named shows.
    pub fn begin_call(&self, label: &str) {
        let now = Instant::now();
        self.with(|a| {
            a.call = Some(label.to_string());
            a.calls += 1;
            a.call_started = Some(now);
        });
    }

    /// An agent subprocess started. Pair with [`ActivityHandle::exit`].
    pub fn enter(&self) {
        let now = Instant::now();
        self.with(|a| {
            a.in_flight += 1;
            a.last_output = Some(now);
        });
    }

    /// An agent subprocess ended. Once none is left, the time since the last
    /// output means nothing until the next one starts, so the clock clears.
    /// Until then it keeps running for the ones still going.
    pub fn exit(&self) {
        self.with(|a| {
            a.in_flight = a.in_flight.saturating_sub(1);
            if a.in_flight == 0 {
                a.call = None;
                a.call_started = None;
                a.last_output = None;
            }
        });
    }

    /// Record the model of a call whose provider does not name it in its
    /// output (opencode): the one it was told to use.
    pub fn set_model(&self, model: &str) {
        self.with(|a| a.model = Some(model.to_string()));
    }

    /// Record that the subprocess wrote something, whatever it was.
    pub fn touch(&self) {
        let now = Instant::now();
        self.with(|a| a.last_output = Some(now));
    }

    /// How long the subprocess has been silent, if a call is running.
    pub fn idle(&self) -> Option<Duration> {
        self.with(|a| a.last_output.map(|t| t.elapsed()))
    }

    /// Record what one stdout line meant, as the provider's dialect read it.
    pub(crate) fn record(&self, event: &AgentEvent) {
        match event {
            AgentEvent::Model(model) => self.with(|a| a.model = Some(model.clone())),
            AgentEvent::Turn { message_id } => self.with(|a| {
                // The CLI may write one event per content block of the same
                // message; a turn without an id counts every time.
                if message_id.is_none() || *message_id != a.last_message_id {
                    a.turns += 1;
                    a.last_message_id = message_id.clone();
                }
            }),
            AgentEvent::Tool(tool) => self.record_tool(tool.clone()),
            AgentEvent::StepFinished(step) => self.record_step_finish(step),
            AgentEvent::StepUsage(_)
            | AgentEvent::Closing { .. }
            | AgentEvent::ErrorEvent(_)
            | AgentEvent::Part { .. } => {}
        }
    }

    /// Log why an opencode model step ended, what it used, and how many
    /// characters it wrote as text, reasoning and tool input. Counts only:
    /// the model's own text can quote a document that is not public, the same
    /// reason `tool_summary` leaves content out.
    fn record_step_finish(&self, step: &StepFinish) {
        self.with(|a| {
            let call = a.call.as_deref().unwrap_or("agent");
            if step.reason == "length" {
                a.steps_cut_off += 1;
                tracing::warn!(
                    call,
                    reason = %step.reason,
                    input = step.input,
                    output = step.output,
                    reasoning = step.reasoning,
                    text_chars = step.parts.text,
                    reasoning_chars = step.parts.reasoning,
                    tool_input_chars = step.parts.tool_input,
                    "agent step cut off by the output limit"
                );
            } else {
                tracing::info!(
                    call,
                    reason = %step.reason,
                    input = step.input,
                    output = step.output,
                    reasoning = step.reasoning,
                    text_chars = step.parts.text,
                    reasoning_chars = step.parts.reasoning,
                    tool_input_chars = step.parts.tool_input,
                    "agent step finished"
                );
            }
        });
    }

    fn record_tool(&self, tool: String) {
        self.with(|a| {
            a.tool_calls += 1;
            tracing::info!(call = a.call.as_deref().unwrap_or("agent"), tool = %tool, "agent tool call");
            a.last_tool = Some(tool.clone());
            if a.recent.len() == RECENT_EVENTS {
                a.recent.pop_front();
            }
            a.recent.push_back(tool);
        });
    }

    /// The activity as JSON, for `jobs.progress`.
    pub fn snapshot(&self) -> serde_json::Value {
        self.with(|a| {
            serde_json::json!({
                "call": a.call,
                "model": a.model,
                "calls": a.calls,
                "call_elapsed_secs": a.call_started.map(|t| t.elapsed().as_secs()),
                "idle_secs": a.last_output.map(|t| t.elapsed().as_secs()),
                "turns": a.turns,
                "tool_calls": a.tool_calls,
                "last_tool": a.last_tool,
                "recent_tools": a.recent,
            })
        })
    }

    /// The tail of an error message: what the agent did before it stopped.
    /// Starts with [`ACTIVITY_MARKER`]; append it last.
    pub fn summary(&self) -> String {
        self.with(|a| {
            let idle = a
                .last_output
                .map(|t| format!(", silent for {}s", t.elapsed().as_secs()))
                .unwrap_or_default();
            let recent = if a.recent.is_empty() {
                String::new()
            } else {
                format!(
                    ", recent tools: {}",
                    a.recent.iter().cloned().collect::<Vec<_>>().join(" | ")
                )
            };
            let step_end = if a.steps_cut_off > 0 {
                format!(", {} step(s) cut off by the output limit", a.steps_cut_off)
            } else {
                String::new()
            };
            format!(
                "{ACTIVITY_MARKER} {} turns, {} tool calls{idle}{recent}{step_end}",
                a.turns, a.tool_calls
            )
        })
    }
}

/// Splits a byte stream into lines, without ever holding more than
/// [`MAX_EVENT_LINE`] bytes of one line.
#[derive(Debug, Default)]
pub struct LineSplitter {
    buf: Vec<u8>,
    /// The current line outgrew the limit; drop bytes until its newline.
    skipping: bool,
}

impl LineSplitter {
    /// Feed a chunk; `on_line` gets every complete line that fit.
    pub fn feed(&mut self, mut chunk: &[u8], mut on_line: impl FnMut(&str)) {
        while let Some(pos) = chunk.iter().position(|b| *b == b'\n') {
            if !self.skipping && self.buf.len() + pos <= MAX_EVENT_LINE {
                self.buf.extend_from_slice(&chunk[..pos]);
                on_line(&String::from_utf8_lossy(&self.buf));
            }
            self.buf.clear();
            self.skipping = false;
            chunk = &chunk[pos + 1..];
        }
        if self.skipping {
            return;
        }
        if self.buf.len() + chunk.len() > MAX_EVENT_LINE {
            self.buf.clear();
            self.skipping = true;
        } else {
            self.buf.extend_from_slice(chunk);
        }
    }
}

/// Count a law YAML's articles and how many carry `machine_readable`, by
/// line shape rather than by parsing: the file is mid-edit while the agent
/// works, and this runs every few seconds on laws of hundreds of articles.
/// Good enough for a progress figure, not for anything that decides.
pub fn count_articles(yaml: &str) -> (usize, usize) {
    let mut articles = 0;
    let mut with_machine_readable = 0;
    for line in yaml.lines() {
        let t = line.trim_start();
        if t.starts_with("- number:") {
            articles += 1;
        } else if t.starts_with("machine_readable:") {
            with_machine_readable += 1;
        }
    }
    (articles, with_machine_readable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::claude::ClaudeDialect;
    use crate::agent::opencode::OpenCodeDialect;
    use crate::agent::AgentDialect;

    /// Record a line as the supervisor does: read by the dialect, then
    /// recorded. The parsing itself is tested next to each dialect.
    fn claude(a: &ActivityHandle, line: &str) {
        for event in ClaudeDialect.read_line(line) {
            a.record(&event);
        }
    }

    fn opencode(a: &ActivityHandle, line: &str) {
        for event in OpenCodeDialect.read_line(line) {
            a.record(&event);
        }
    }

    #[test]
    fn steps_cut_off_by_the_output_limit_are_counted_across_calls() {
        let a = ActivityHandle::new();
        let cut = r#"{"type":"step_finish","part":{"reason":"length","tokens":{"output":32000}}}"#;
        opencode(&a, cut);
        // A later call that ends normally must not hide the cut-off one.
        opencode(&a, r#"{"type":"step_finish","part":{"reason":"stop"}}"#);
        assert!(
            a.summary()
                .contains("1 step(s) cut off by the output limit"),
            "{}",
            a.summary()
        );
        opencode(&a, cut);
        assert!(a.summary().contains("2 step(s) cut off"), "{}", a.summary());
        let clean = ActivityHandle::new();
        opencode(&clean, r#"{"type":"step_finish","part":{"reason":"stop"}}"#);
        assert!(!clean.summary().contains("cut off"), "{}", clean.summary());
    }

    #[test]
    fn opencode_steps_and_tool_calls_are_counted() {
        let a = ActivityHandle::new();
        opencode(&a, r#"{"type":"step_start","part":{"type":"step-start"}}"#);
        opencode(
            &a,
            r#"{"type":"tool_use","part":{"type":"tool","tool":"edit","state":{"status":"completed","input":{"filePath":"law.yaml","oldString":"secret"}}}}"#,
        );
        opencode(
            &a,
            r#"{"type":"tool_use","part":{"type":"tool","tool":"write","state":{"status":"error"}}}"#,
        );
        let summary = a.summary();
        assert!(summary.contains("1 turns, 2 tool calls"), "{summary}");
        assert!(
            summary.contains("edit law.yaml | write (error)"),
            "{summary}"
        );
        assert!(!summary.contains("secret"), "{summary}");
    }

    fn assistant(id: &str, content: &str) -> String {
        format!(r#"{{"type":"assistant","message":{{"id":"{id}","content":[{content}]}}}}"#)
    }

    #[test]
    fn counts_turns_by_message_and_tools_by_block() {
        let a = ActivityHandle::new();
        a.begin_call("translate");
        claude(
            &a,
            &assistant("m1", r#"{"type":"text","text":"Reading the law"}"#),
        );
        // The CLI may emit one event per content block of the same message.
        claude(
            &a,
            &assistant(
                "m1",
                r#"{"type":"tool_use","name":"Read","input":{"file_path":"laws/x/law.yaml"}}"#,
            ),
        );
        claude(
            &a,
            &assistant(
                "m2",
                r#"{"type":"tool_use","name":"Edit","input":{"file_path":"laws/x/law.yaml","old_string":"a","new_string":"b"}},
               {"type":"tool_use","name":"Glob","input":{"path":"laws"}}"#,
            ),
        );
        let s = a.snapshot();
        assert_eq!(s["call"], "translate");
        assert_eq!(s["turns"], 2);
        assert_eq!(s["tool_calls"], 3);
        assert_eq!(s["last_tool"], "Glob laws");
        assert_eq!(
            s["recent_tools"],
            serde_json::json!(["Read laws/x/law.yaml", "Edit laws/x/law.yaml", "Glob laws"])
        );
    }

    #[test]
    fn other_events_change_nothing() {
        let a = ActivityHandle::new();
        claude(
            &a,
            r#"{"type":"stream_event","event":{"type":"content_block_delta"}}"#,
        );
        claude(
            &a,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#,
        );
        claude(&a, r#"{"type":"result","usage":{"input_tokens":1}}"#);
        claude(&a, "not json");
        let s = a.snapshot();
        assert_eq!(s["turns"], 0);
        assert_eq!(s["tool_calls"], 0);
    }

    #[test]
    fn the_snapshot_names_the_model() {
        let a = ActivityHandle::new();
        assert!(a.snapshot()["model"].is_null());
        claude(
            &a,
            r#"{"type":"system","subtype":"init","model":"claude-opus-5-5"}"#,
        );
        assert_eq!(a.snapshot()["model"], "claude-opus-5-5");
    }

    #[test]
    fn the_summary_is_cut_off_before_classifying() {
        let a = ActivityHandle::new();
        claude(
            &a,
            &assistant(
                "m1",
                r#"{"type":"tool_use","name":"Read","input":{"file_path":"yaml error os error 12"}}"#,
            ),
        );
        let err = format!("claude timed out after 5s; stderr: boom{}", a.summary());
        assert_eq!(
            without_activity(&err),
            "claude timed out after 5s; stderr: boom"
        );
        assert_eq!(without_activity("no summary here"), "no summary here");
    }

    #[test]
    fn exit_clears_the_clock_once_no_call_is_left() {
        let a = ActivityHandle::new();
        a.begin_call("translate");
        a.enter();
        a.exit();
        assert!(a.idle().is_none());
        let s = a.snapshot();
        assert!(s["call"].is_null());
        assert!(s["call_elapsed_secs"].is_null());
        assert_eq!(s["calls"], 1);
    }

    #[test]
    fn a_window_that_ends_keeps_the_clock_for_the_ones_still_running() {
        // Two windows side by side: the first to finish must not switch off
        // the stall check for the other.
        let a = ActivityHandle::new();
        a.begin_call("translate");
        a.enter();
        a.begin_call("translate");
        a.enter();
        a.exit();
        assert!(a.idle().is_some(), "the other window is still running");
        assert_eq!(a.snapshot()["call"], "translate");
        a.exit();
        assert!(a.idle().is_none());
        // An extra exit does not wrap the count around.
        a.exit();
        a.enter();
        a.exit();
        assert!(a.idle().is_none());
    }

    #[test]
    fn summary_names_turns_tools_and_recent_work() {
        let a = ActivityHandle::new();
        assert_eq!(a.summary(), "; agent activity: 0 turns, 0 tool calls");
        a.begin_call("translate");
        a.enter();
        claude(
            &a,
            &assistant(
                "m1",
                r#"{"type":"tool_use","name":"Read","input":{"file_path":"a.yaml"}}"#,
            ),
        );
        let s = a.summary();
        assert!(
            s.starts_with("; agent activity: 1 turns, 1 tool calls, silent for "),
            "{s}"
        );
        assert!(s.ends_with(", recent tools: Read a.yaml"), "{s}");
    }

    #[test]
    fn recent_tools_keep_only_the_latest() {
        let a = ActivityHandle::new();
        for i in 0..(RECENT_EVENTS + 3) {
            claude(
                &a,
                &assistant(
                    &format!("m{i}"),
                    &format!(
                        r#"{{"type":"tool_use","name":"Read","input":{{"file_path":"f{i}"}}}}"#
                    ),
                ),
            );
        }
        let s = a.snapshot();
        let recent = s["recent_tools"].as_array().unwrap();
        assert_eq!(recent.len(), RECENT_EVENTS);
        assert_eq!(recent[0], "Read f3");
        assert_eq!(s["tool_calls"], (RECENT_EVENTS + 3) as u64);
    }

    #[test]
    fn begin_call_names_and_counts_calls() {
        let a = ActivityHandle::new();
        a.begin_call("translate");
        a.begin_call("schema");
        let s = a.snapshot();
        assert_eq!(s["calls"], 2);
        assert_eq!(s["call"], "schema");
        // Naming a call starts no clock; a running subprocess does.
        assert!(a.idle().is_none());
        a.enter();
        assert!(a.idle().unwrap() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn touch_resets_silence() {
        let a = ActivityHandle::new();
        a.enter();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(a.idle().unwrap() >= Duration::from_millis(200));
        a.touch();
        assert!(a.idle().unwrap() < Duration::from_millis(100));
    }

    #[test]
    fn splitter_yields_lines_across_chunks() {
        let mut s = LineSplitter::default();
        let mut lines = Vec::new();
        s.feed(b"one\ntw", |l| lines.push(l.to_string()));
        s.feed(b"o\nthree", |l| lines.push(l.to_string()));
        s.feed(b"\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, ["one", "two", "three"]);
    }

    #[test]
    fn splitter_drops_an_oversized_line_and_recovers() {
        let mut s = LineSplitter::default();
        let mut lines = Vec::new();
        let big = vec![b'x'; MAX_EVENT_LINE + 10];
        s.feed(b"before\n", |l| lines.push(l.to_string()));
        s.feed(&big, |l| lines.push(l.to_string()));
        s.feed(b"still the big one\nafter\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, ["before", "after"]);
    }

    #[test]
    fn splitter_keeps_a_line_of_exactly_the_limit() {
        let mut s = LineSplitter::default();
        let mut n = 0;
        let mut line = vec![b'x'; MAX_EVENT_LINE];
        line.push(b'\n');
        s.feed(&line, |l| {
            assert_eq!(l.len(), MAX_EVENT_LINE);
            n += 1;
        });
        assert_eq!(n, 1);
    }

    #[test]
    fn counts_articles_and_machine_readable_by_line() {
        let yaml = "\
articles:
  - number: '1'
    text: Begrippen
    machine_readable:
      definitions: {}
  - number: '2'
    text: Recht op zorgtoeslag
  - number: 2a
    machine_readable:
      execution: {}
";
        assert_eq!(count_articles(yaml), (3, 2));
        assert_eq!(count_articles(""), (0, 0));
    }
}
