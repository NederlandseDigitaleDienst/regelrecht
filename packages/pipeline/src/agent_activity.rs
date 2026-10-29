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
//! silent). Any provider counts as active while it writes to stdout; only the
//! claude events are read in detail.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

/// How many recent events a snapshot and an error message carry.
const RECENT_EVENTS: usize = 8;

/// The longest event line that is parsed. Longer lines (a tool result that
/// inlines a whole file) still count as activity but are not read, so one
/// large event cannot make the worker hold megabytes.
pub const MAX_EVENT_LINE: usize = 1024 * 1024;

/// The longest tool target kept in a summary.
const MAX_TARGET_CHARS: usize = 120;

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
    /// The model the CLI said it runs, from its `init` event.
    model: Option<String>,
    recent: VecDeque<String>,
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

    /// Record that the subprocess wrote something, whatever it was.
    pub fn touch(&self) {
        let now = Instant::now();
        self.with(|a| a.last_output = Some(now));
    }

    /// How long the subprocess has been silent, if a call is running.
    pub fn idle(&self) -> Option<Duration> {
        self.with(|a| a.last_output.map(|t| t.elapsed()))
    }

    /// Read one complete stdout line. Lines that are not claude events are
    /// ignored; they already counted as activity when they arrived.
    pub fn record_line(&self, line: &str) {
        // Partial message chunks arrive per token. They keep the run visibly
        // alive, which is their whole use here; parsing each would only cost.
        if line.starts_with(r#"{"type":"stream_event""#) {
            return;
        }
        if let Some(model) = init_model(line) {
            self.with(|a| a.model = Some(model));
            return;
        }
        // Only assistant messages are read. A tool result can be a whole
        // file; skip parsing what cannot be one. The CLI does not promise
        // key order, so look for the pair anywhere; `classify` checks the
        // top level.
        if !line.contains(r#""type":"assistant""#) {
            return;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        let Some(Event::Assistant { message_id, tools }) = classify(&event) else {
            return;
        };
        self.with(|a| {
            if message_id.is_none() || message_id != a.last_message_id {
                a.turns += 1;
                a.last_message_id = message_id;
            }
            for tool in tools {
                a.tool_calls += 1;
                tracing::info!(call = a.call.as_deref().unwrap_or("agent"), tool = %tool, "agent tool call");
                a.last_tool = Some(tool.clone());
                if a.recent.len() == RECENT_EVENTS {
                    a.recent.pop_front();
                }
                a.recent.push_back(tool);
            }
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
            format!(
                "{ACTIVITY_MARKER} {} turns, {} tool calls{idle}{recent}",
                a.turns, a.tool_calls
            )
        })
    }
}

/// Whether `line` is the claude provider's closing `result` event, which
/// carries the usage. Checked on the parsed top level: the CLI does not put
/// `type` first, and a tool result can quote the pair.
pub fn is_result_event(line: &str) -> bool {
    line.contains(r#""type":"result""#)
        && serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| v.get("type")?.as_str().map(|t| t == "result"))
            .unwrap_or(false)
}

/// The model named in the claude CLI's `init` event, the first line of a
/// stream-json run. This is what actually runs: the worker passes a model only
/// when `CLAUDE_MODEL` is set, and otherwise the CLI picks one from its version
/// and the subscription behind the token.
pub fn init_model(line: &str) -> Option<String> {
    if !line.contains(r#""subtype":"init""#) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "system" || v.get("subtype")?.as_str()? != "init" {
        return None;
    }
    v.get("model")?
        .as_str()
        .filter(|m| !m.is_empty())
        .map(str::to_string)
}

/// The longest CLI error text kept in a job's error message.
const MAX_ERROR_TEXT_CHARS: usize = 300;

/// The error the claude CLI reported in its closing `result` event, if it
/// reported one. A failed API call (rate limit, overload, an expired token)
/// ends the run with `is_error` set and the reason as text in `result`; the
/// process then exits 1 with nothing on stderr, so this is the only place
/// the reason is written down. Some error subtypes carry an `errors` list
/// instead. One line, cut to a limit, for appending to the job's error.
pub fn result_error_text(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "result" {
        return None;
    }
    let subtype = v.get("subtype").and_then(serde_json::Value::as_str);
    let is_error = v.get("is_error").and_then(serde_json::Value::as_bool) == Some(true)
        || subtype.is_some_and(|s| s.starts_with("error"));
    if !is_error {
        return None;
    }
    let from_result = v
        .get("result")
        .and_then(serde_json::Value::as_str)
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string);
    let from_errors = || {
        let list: Vec<&str> = v
            .get("errors")?
            .as_array()?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        (!list.is_empty()).then(|| list.join("; "))
    };
    let text = from_result
        .or_else(from_errors)
        .or_else(|| subtype.map(str::to_string))?;
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = one_line.chars().take(MAX_ERROR_TEXT_CHARS).collect();
    Some(if short.len() < one_line.len() {
        format!("{short}…")
    } else {
        short
    })
}

/// The parts of a claude stream-json event this module reads.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Assistant {
        message_id: Option<String>,
        /// One summary per `tool_use` block, e.g. `Edit laws/x/law.yaml`.
        tools: Vec<String>,
    },
}

fn classify(event: &serde_json::Value) -> Option<Event> {
    if event.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let message = event.get("message")?;
    let message_id = message
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let tools = message
        .get("content")
        .and_then(serde_json::Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(serde_json::Value::as_str) == Some("tool_use"))
                .map(tool_summary)
                .collect()
        })
        .unwrap_or_default();
    Some(Event::Assistant { message_id, tools })
}

/// `Name target`, where the target is the file, URL or skill the tool works
/// on. Only names the worker or the corpus already has: never the content of
/// a write, and never a search pattern or a shell command, which the model
/// composes and which can quote an uploaded document that is not public.
fn tool_summary(block: &serde_json::Value) -> String {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool");
    let input = block.get("input");
    let target = ["file_path", "path", "url", "skill"]
        .iter()
        .find_map(|key| input?.get(*key)?.as_str());
    match target {
        Some(t) => {
            let t = t.lines().next().unwrap_or_default();
            let short: String = t.chars().take(MAX_TARGET_CHARS).collect();
            if short.len() < t.len() {
                format!("{name} {short}…")
            } else {
                format!("{name} {short}")
            }
        }
        None => name.to_string(),
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

    fn assistant(id: &str, content: &str) -> String {
        format!(r#"{{"type":"assistant","message":{{"id":"{id}","content":[{content}]}}}}"#)
    }

    #[test]
    fn counts_turns_by_message_and_tools_by_block() {
        let a = ActivityHandle::new();
        a.begin_call("translate");
        a.record_line(&assistant(
            "m1",
            r#"{"type":"text","text":"Reading the law"}"#,
        ));
        // The CLI may emit one event per content block of the same message.
        a.record_line(&assistant(
            "m1",
            r#"{"type":"tool_use","name":"Read","input":{"file_path":"laws/x/law.yaml"}}"#,
        ));
        a.record_line(&assistant(
            "m2",
            r#"{"type":"tool_use","name":"Edit","input":{"file_path":"laws/x/law.yaml","old_string":"a","new_string":"b"}},
               {"type":"tool_use","name":"Glob","input":{"path":"laws"}}"#,
        ));
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
        a.record_line(r#"{"type":"stream_event","event":{"type":"content_block_delta"}}"#);
        a.record_line(r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#);
        a.record_line(r#"{"type":"result","usage":{"input_tokens":1}}"#);
        a.record_line("not json");
        let s = a.snapshot();
        assert_eq!(s["turns"], 0);
        assert_eq!(s["tool_calls"], 0);
    }

    #[test]
    fn a_tool_summary_never_carries_written_content() {
        let block = serde_json::json!({
            "type": "tool_use", "name": "Write",
            "input": {"file_path": "laws/x/law.yaml", "content": "secret law text"}
        });
        assert_eq!(tool_summary(&block), "Write laws/x/law.yaml");
    }

    #[test]
    fn a_long_target_is_cut_to_its_first_line_and_a_limit() {
        let long = "x".repeat(300);
        let block =
            serde_json::json!({"name": "Read", "input": {"file_path": format!("{long}\nsecond")}});
        let s = tool_summary(&block);
        assert!(s.starts_with("Read xxx"));
        assert!(s.ends_with('…'));
        assert_eq!(s.chars().count(), "Read ".len() + MAX_TARGET_CHARS + 1);

        let short =
            serde_json::json!({"name": "WebFetch", "input": {"url": "https://example.org\nmore"}});
        assert_eq!(tool_summary(&short), "WebFetch https://example.org");

        let bare = serde_json::json!({"name": "TodoWrite", "input": {}});
        assert_eq!(tool_summary(&bare), "TodoWrite");
    }

    #[test]
    fn patterns_and_commands_stay_out_of_a_summary() {
        let grep = serde_json::json!({"name": "Grep", "input": {"pattern": "a quoted phrase"}});
        assert_eq!(tool_summary(&grep), "Grep");
        let bash =
            serde_json::json!({"name": "Bash", "input": {"command": "grep 'draft text' doc.md"}});
        assert_eq!(tool_summary(&bash), "Bash");
    }

    #[test]
    fn the_result_event_is_found_whatever_the_key_order() {
        assert!(is_result_event(
            r#"{"duration_api_ms":1,"is_error":false,"type":"result","usage":{}}"#
        ));
        assert!(!is_result_event(
            r#"{"type":"user","message":{"content":[{"text":"\"type\":\"result\""}]}}"#
        ));
        assert!(!is_result_event(
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#
        ));
        assert!(!is_result_event("not json"));
    }

    #[test]
    fn the_cli_error_is_read_from_the_result_event() {
        let line = r#"{"duration_ms":240000,"is_error":true,"type":"result","subtype":"success","result":"API Error: 429 {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}"}"#;
        assert_eq!(
            result_error_text(line).as_deref(),
            Some(r#"API Error: 429 {"type":"error","error":{"type":"rate_limit_error"}}"#)
        );
    }

    #[test]
    fn an_error_subtype_without_text_falls_back_to_its_errors_or_its_name() {
        let with_errors = r#"{"type":"result","subtype":"error_during_execution","is_error":false,"errors":["first","second"]}"#;
        assert_eq!(
            result_error_text(with_errors).as_deref(),
            Some("first; second")
        );
        let bare = r#"{"type":"result","subtype":"error_max_turns","is_error":false}"#;
        assert_eq!(result_error_text(bare).as_deref(), Some("error_max_turns"));
    }

    #[test]
    fn a_successful_result_has_no_error_text() {
        let ok = r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#;
        assert_eq!(result_error_text(ok), None);
        assert_eq!(result_error_text(r#"{"type":"assistant"}"#), None);
        assert_eq!(result_error_text("not json"), None);
    }

    #[test]
    fn the_cli_error_becomes_one_short_line() {
        let long = format!("API Error:\n  {}", "x".repeat(500));
        let line =
            serde_json::json!({"type": "result", "is_error": true, "result": long}).to_string();
        let text = result_error_text(&line).unwrap();
        assert!(text.starts_with("API Error: xxx"), "{text}");
        assert!(!text.contains('\n'));
        assert!(text.ends_with('…'));
        assert_eq!(text.chars().count(), MAX_ERROR_TEXT_CHARS + 1);
    }

    #[test]
    fn the_model_is_read_from_the_init_event() {
        let init = r#"{"type":"system","subtype":"init","cwd":"/tmp","model":"claude-opus-5-5","tools":["Read"]}"#;
        assert_eq!(init_model(init).as_deref(), Some("claude-opus-5-5"));
        // Any key order, as with the other events.
        let reordered = r#"{"model":"claude-sonnet-5-5","subtype":"init","type":"system"}"#;
        assert_eq!(init_model(reordered).as_deref(), Some("claude-sonnet-5-5"));
        // Other system events, a quoted init in a tool result, and an empty name
        // say nothing about the model.
        assert_eq!(
            init_model(r#"{"type":"system","subtype":"status","model":"x"}"#),
            None
        );
        assert_eq!(
            init_model(r#"{"type":"user","message":{"content":"\"subtype\":\"init\""}}"#),
            None
        );
        assert_eq!(
            init_model(r#"{"type":"system","subtype":"init","model":""}"#),
            None
        );
    }

    #[test]
    fn the_snapshot_names_the_model() {
        let a = ActivityHandle::new();
        assert!(a.snapshot()["model"].is_null());
        a.record_line(r#"{"type":"system","subtype":"init","model":"claude-opus-5-5"}"#);
        assert_eq!(a.snapshot()["model"], "claude-opus-5-5");
    }

    #[test]
    fn an_assistant_event_is_read_whatever_the_key_order() {
        let a = ActivityHandle::new();
        a.record_line(
            r#"{"message":{"id":"m1","content":[{"type":"tool_use","name":"Read","input":{"file_path":"a"}}]},"type":"assistant"}"#,
        );
        assert_eq!(a.snapshot()["tool_calls"], 1);
    }

    #[test]
    fn the_summary_is_cut_off_before_classifying() {
        let a = ActivityHandle::new();
        a.record_line(&assistant(
            "m1",
            r#"{"type":"tool_use","name":"Read","input":{"file_path":"yaml error os error 12"}}"#,
        ));
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
        a.record_line(&assistant(
            "m1",
            r#"{"type":"tool_use","name":"Read","input":{"file_path":"a.yaml"}}"#,
        ));
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
            a.record_line(&assistant(
                &format!("m{i}"),
                &format!(r#"{{"type":"tool_use","name":"Read","input":{{"file_path":"f{i}"}}}}"#),
            ));
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
