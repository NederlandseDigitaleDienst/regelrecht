//! The claude CLI, run with `--output-format stream-json`.
//!
//! It prints one JSON event per line as the session goes: the `init` naming
//! the model, every assistant message, every tool result, a partial message
//! event per token, and a closing `result` with the usage and, on failure, the
//! reason. The CLI does not put `type` first in an event, so every check looks
//! for the pair anywhere in the line and then confirms it on the parsed top
//! level.

use super::dialect::{
    AgentDialect, AgentEvent, Capabilities, CommandRequest, CommandSpec, ToolPolicy,
};
use crate::enrich::{claude_model, AgentUsage, LlmProvider, SessionAction};

/// The longest tool target kept in a summary.
const MAX_TARGET_CHARS: usize = 120;

/// The longest CLI error text kept in a job's error message.
const MAX_ERROR_TEXT_CHARS: usize = 300;

pub(crate) struct ClaudeDialect;

impl AgentDialect for ClaudeDialect {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            resumes_sessions: true,
            // `--include-partial-messages` writes an event per token.
            streams_per_token: true,
            names_model: true,
        }
    }

    fn command(&self, req: &CommandRequest<'_>) -> CommandSpec {
        let mut cmd = CommandSpec::new(req.program);
        let token = req
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CODE_OAUTH_TOKEN")
            .map(|(_, v)| v.as_str());
        // Classify the token by its non-secret prefix, so a misconfiguration
        // (an API key pasted into the OAuth slot) is obvious in the log.
        tracing::info!(
            claude_oauth_token_kind = ?token.map(token_kind),
            "claude credentials"
        );
        // If CLAUDE_CODE_OAUTH_TOKEN holds several comma-separated tokens,
        // override the forwarded value with a single one chosen by a
        // time-rotating index, so load spreads across the subscriptions.
        if let Some(raw) = token {
            if let Some((idx, count, token)) = select_claude_token(raw, req.rotation_bucket) {
                // Always apply the selected (trimmed) token — even for a
                // single token — so stray whitespace or a trailing comma
                // never reaches claude verbatim. Never log the token value;
                // only the 1-based position and count.
                cmd.env
                    .push(("CLAUDE_CODE_OAUTH_TOKEN".into(), token.to_string()));
                if count > 1 {
                    tracing::info!(
                        using_token = idx + 1,
                        of_tokens = count,
                        "selected claude oauth token (rotating by ~100s)"
                    );
                }
            }
        }
        // Leaving a tool off `--allowedTools` does not take it away: that
        // flag auto-approves, and what it omits merely has to be asked for,
        // which the checkout's own settings then grant. Withholding needs
        // `--disallowedTools`. Without this the enrichment agent made
        // twenty `Bash` calls in a session whose plan reported the step as
        // running degraded for want of a shell.
        let denied = req.tools.denied();
        cmd.arg("-p")
            .arg(req.prompt)
            .arg("--allowedTools")
            .arg(req.tools.allowed())
            // One JSON event per line while the run goes, closing with
            // the `result` event that carries the token use. The worker
            // reads the events for progress and to notice a stalled
            // run; `--include-partial-messages` adds an event per token
            // so a long answer does not look like silence. stream-json
            // requires `--verbose` under `-p`.
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--include-partial-messages");
        cmd.current_dir = Some(req.cwd.to_path_buf());
        if !denied.is_empty() {
            cmd.arg("--disallowedTools").arg(denied.join(","));
        }
        if let Some(m) = req.model {
            cmd.arg("--model").arg(m);
        }
        // Effort rides beside the model because that is how the CLI takes
        // it: one flag for which model, one for how hard it thinks.
        if let Some(e) = req.effort {
            cmd.arg("--effort").arg(e);
        }
        // The window's session. The worker chooses the id rather than
        // reading it back out of the provider's closing JSON: an id we
        // hand in is known before the process starts, so a run that dies
        // before it prints anything still leaves a resumable session, and
        // nothing has to be parsed to continue.
        match req.session {
            SessionAction::Cold => {}
            SessionAction::Start(id) => {
                cmd.arg("--session-id").arg(id.to_string());
            }
            SessionAction::Resume(id) => {
                cmd.arg("--resume").arg(id.to_string());
            }
        }
        cmd
    }

    fn read_line(&self, line: &str) -> Vec<AgentEvent> {
        // Partial message chunks arrive per token. They keep the run visibly
        // alive, which is their whole use here; parsing each would only cost.
        if line.starts_with(r#"{"type":"stream_event""#) {
            return Vec::new();
        }
        if let Some(model) = init_model(line) {
            return vec![AgentEvent::Model(model)];
        }
        if is_result_event(line) {
            // Read from the line itself, kept whole: a long closing event can
            // be cut in half by the bounded stdout tail.
            return vec![AgentEvent::Closing {
                usage: usage_from_json(line),
                error: result_error_text(line),
            }];
        }
        // Only assistant messages are read further. A tool result can be a
        // whole file; skip parsing what cannot be one.
        if !line.contains(r#""type":"assistant""#) {
            return Vec::new();
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return Vec::new();
        };
        let Some((message_id, tools)) = assistant(&event) else {
            return Vec::new();
        };
        let mut out = vec![AgentEvent::Turn { message_id }];
        out.extend(tools.into_iter().map(AgentEvent::Tool));
        out
    }

    fn usage_from_tail(&self, tail: &str) -> Option<AgentUsage> {
        usage_from_json(tail)
    }
}

/// How claude's two tool flags say what a [`ToolPolicy`] allows.
impl ToolPolicy<'_> {
    /// The claude provider's `--allowedTools` value.
    pub(crate) fn allowed(self) -> &'static str {
        if self.allow_bash {
            "Bash,Read,Edit,Write,Grep,Glob"
        } else {
            "Read,Edit,Write,Grep,Glob"
        }
    }

    /// The claude provider's `--disallowedTools` value, empty when nothing is
    /// withheld. A caller that did not ask for the shell does not merely fail
    /// to get it approved: it does not get it.
    pub(crate) fn denied(self) -> Vec<String> {
        let mut out: Vec<String> = self.deny.to_vec();
        if !self.allow_bash {
            out.push("Bash".to_owned());
        }
        out.sort();
        out.dedup();
        out
    }
}

/// The claude provider as `from_env` builds it: `CLAUDE_PATH` or `LLM_PATH`
/// for the binary, and the model from [`claude_model`].
pub(crate) fn provider_from_env(var: impl Fn(&str) -> Option<String>) -> LlmProvider {
    LlmProvider::Claude {
        path: var("CLAUDE_PATH")
            .or_else(|| var("LLM_PATH"))
            .unwrap_or_else(|| "claude".into())
            .into(),
        model: Some(claude_model(var("CLAUDE_MODEL"), var("LLM_MODEL"))),
    }
}

fn token_kind(token: &str) -> &'static str {
    if token.is_empty() {
        "empty"
    } else if token.starts_with("sk-ant-oat") {
        "oauth-token (sk-ant-oat…)"
    } else if token.starts_with("sk-ant-api") {
        "WRONG: looks like an API key (sk-ant-api…)"
    } else {
        "unrecognized-prefix"
    }
}

/// Select one Claude OAuth token from a comma-separated list, rotating by a
/// time `bucket` so consecutive runs spread across several personal
/// subscriptions (each token has its own usage/rate limits).
///
/// `CLAUDE_CODE_OAUTH_TOKEN` may hold multiple tokens separated by commas. The
/// chosen index is `bucket % n`; callers pass `unix_secs / 100`, so the active
/// token rotates roughly every 100 seconds. Returns `(index, count, token)`, or
/// `None` when there are no non-empty tokens.
fn select_claude_token(raw: &str, bucket: u64) -> Option<(usize, usize, &str)> {
    let tokens: Vec<&str> = raw
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let idx = (bucket % tokens.len() as u64) as usize;
    Some((idx, tokens.len(), tokens[idx]))
}

/// Read the usage out of the CLI's final JSON object.
///
/// Takes the tail of stdout rather than the whole stream: the object we
/// want is the last one. Returns `None` when the tail carries no recognisable
/// object, which is the normal case for a provider that reports nothing.
fn usage_from_json(tail: &str) -> Option<AgentUsage> {
    // The whole tail first. `--output-format json` writes one object, and
    // looking for the last `{` lands inside it: this payload nests
    // `iterations` and `cache_creation`, so the scan found a fragment with
    // no `usage` and every figure came out zero.
    //
    // The scan stays for the streaming shape, where the closing object
    // really is the last one on the stream.
    //
    // Then line by line from the end, for stream-json: the last line
    // with a top-level `usage` is the closing `result`. Assistant and
    // stream events carry theirs nested, and the CLI puts `type`
    // anywhere in the object, so neither key order nor a `{"type"`
    // prefix can be relied on.
    let value: serde_json::Value = serde_json::from_str(tail.trim())
        .ok()
        .or_else(|| {
            tail.lines().rev().find_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
                v.get("usage").is_some().then_some(v)
            })
        })
        .or_else(|| {
            let start = tail.rfind("{\"type\"").or_else(|| tail.rfind('{'))?;
            serde_json::from_str(tail[start..].trim()).ok()
        })?;
    let usage = value.get("usage")?;
    let n = |key: &str| {
        usage
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    Some(AgentUsage {
        input_tokens: n("input_tokens"),
        output_tokens: n("output_tokens"),
        cache_read_tokens: n("cache_read_input_tokens"),
        cache_write_tokens: n("cache_creation_input_tokens"),
        cost_millicents: value
            .get("total_cost_usd")
            .and_then(serde_json::Value::as_f64)
            .map_or(0, |usd| (usd * 100_000.0).round() as u64),
    })
}

/// Whether `line` is the closing `result` event, which carries the usage.
/// Checked on the parsed top level: a tool result can quote the pair.
fn is_result_event(line: &str) -> bool {
    line.contains(r#""type":"result""#)
        && serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| v.get("type")?.as_str().map(|t| t == "result"))
            .unwrap_or(false)
}

/// The model named in the `init` event, the first line of a stream-json run.
/// This is what actually runs, as the CLI reports it; the worker asks for
/// `CLAUDE_MODEL` or `DEFAULT_CLAUDE_MODEL` in `enrich.rs`.
fn init_model(line: &str) -> Option<String> {
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

/// The error the CLI reported in its closing `result` event, if it reported
/// one. A failed API call (rate limit, overload, an expired token) ends the
/// run with `is_error` set and the reason as text in `result`; the process
/// then exits 1 with nothing on stderr, so this is the only place the reason
/// is written down. Some error subtypes carry an `errors` list instead. One
/// line, cut to a limit, for appending to the job's error.
fn result_error_text(line: &str) -> Option<String> {
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

/// The message id and one summary per `tool_use` block of an assistant
/// event, or `None` for any other event.
fn assistant(event: &serde_json::Value) -> Option<(Option<String>, Vec<String>)> {
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
    Some((message_id, tools))
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use uuid::Uuid;

    use super::*;

    fn assistant_line(id: &str, content: &str) -> String {
        format!(r#"{{"type":"assistant","message":{{"id":"{id}","content":[{content}]}}}}"#)
    }

    fn command(
        tools: ToolPolicy<'_>,
        session: SessionAction,
        env: &[(String, String)],
    ) -> CommandSpec {
        ClaudeDialect.command(&CommandRequest {
            program: Path::new("claude"),
            model: None,
            prompt: "prompt",
            file_arg: None,
            cwd: Path::new("/tmp"),
            tools,
            effort: None,
            session,
            env,
            rotation_bucket: 0,
        })
    }

    const NO_TOOLS: ToolPolicy<'static> = ToolPolicy {
        allow_bash: false,
        deny: &[],
    };

    #[test]
    fn a_tool_the_lane_does_not_grant_is_denied_and_not_merely_left_off() {
        // `--allowedTools` auto-approves; it does not withhold. Leaving Bash
        // off it is what let the enrichment agent make twenty shell calls in a
        // session whose plan reported the shell as absent. Whatever the
        // capability planner calls ungranted has to arrive as `--disallowedTools`
        // or the plan is a description of a runtime that does not exist.
        let args = |allow_bash: bool, deny: &[String]| -> Vec<String> {
            command(ToolPolicy { allow_bash, deny }, SessionAction::Cold, &[]).args_lossy()
        };

        let lane = args(false, &["WebFetch".to_owned(), "WebSearch".to_owned()]);
        let at = lane
            .iter()
            .position(|a| a == "--disallowedTools")
            .expect("lane denies something");
        assert_eq!(lane[at + 1], "Bash,WebFetch,WebSearch");

        // A caller that asked for the shell keeps it.
        let converter = args(true, &[]);
        assert!(
            !converter.iter().any(|a| a == "--disallowedTools"),
            "document-convert must keep its shell: {converter:?}"
        );
    }

    #[test]
    fn the_claude_command_starts_and_resumes_a_session() {
        let id = Uuid::new_v4();
        let args = |action: SessionAction| command(NO_TOOLS, action, &[]).args_lossy();

        let cold = args(SessionAction::Cold);
        assert!(!cold.iter().any(|a| a == "--session-id" || a == "--resume"));

        let start = args(SessionAction::Start(id));
        let at = start
            .iter()
            .position(|a| a == "--session-id")
            .expect("start");
        assert_eq!(start[at + 1], id.to_string());

        let resume = args(SessionAction::Resume(id));
        let at = resume.iter().position(|a| a == "--resume").expect("resume");
        assert_eq!(resume[at + 1], id.to_string());
    }

    #[test]
    fn the_command_runs_in_the_working_directory_and_streams() {
        let cmd = command(NO_TOOLS, SessionAction::Cold, &[]);
        assert_eq!(cmd.current_dir.as_deref(), Some(Path::new("/tmp")));
        let args = cmd.args_lossy();
        assert_eq!(args[..2], ["-p", "prompt"]);
        assert!(args
            .windows(2)
            .any(|w| w == ["--output-format", "stream-json"]));
        assert!(args.iter().any(|a| a == "--include-partial-messages"));
    }

    #[test]
    fn one_token_of_several_is_passed_on_trimmed() {
        let env = [("CLAUDE_CODE_OAUTH_TOKEN".to_string(), " a, b ".to_string())];
        let cmd = command(NO_TOOLS, SessionAction::Cold, &env);
        assert_eq!(
            cmd.env,
            [("CLAUDE_CODE_OAUTH_TOKEN".to_string(), "a".to_string())]
        );
        // Without a token nothing is overridden.
        assert!(command(NO_TOOLS, SessionAction::Cold, &[]).env.is_empty());
    }

    #[test]
    fn test_select_claude_token() {
        // empty / whitespace-only -> None
        assert_eq!(select_claude_token("", 0), None);
        assert_eq!(select_claude_token("  , ,", 5), None);

        // single token -> always that token, index 0
        assert_eq!(select_claude_token("tokA", 0), Some((0, 1, "tokA")));
        assert_eq!(select_claude_token(" tokA ", 999), Some((0, 1, "tokA")));

        // multiple tokens -> rotate by bucket % n, whitespace trimmed
        assert_eq!(select_claude_token("a, b , c", 0), Some((0, 3, "a")));
        assert_eq!(select_claude_token("a, b , c", 1), Some((1, 3, "b")));
        assert_eq!(select_claude_token("a, b , c", 2), Some((2, 3, "c")));
        assert_eq!(select_claude_token("a, b , c", 3), Some((0, 3, "a")));
        // large bucket (e.g. unix_secs/100) still wraps correctly
        assert_eq!(select_claude_token("a,b", 17_000_001), Some((1, 2, "b")));
    }

    #[test]
    fn the_provider_is_read_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(name, _)| *name == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let LlmProvider::Claude { path, model } = provider_from_env(env(&[])) else {
            panic!("a claude provider")
        };
        assert_eq!(path, Path::new("claude"));
        assert_eq!(model.as_deref(), Some(crate::enrich::DEFAULT_CLAUDE_MODEL));

        let LlmProvider::Claude { path, model } = provider_from_env(env(&[
            ("LLM_PATH", "/bin/llm"),
            ("LLM_MODEL", "fallback"),
            ("CLAUDE_MODEL", "chosen"),
        ])) else {
            panic!("a claude provider")
        };
        assert_eq!(path, Path::new("/bin/llm"));
        assert_eq!(model.as_deref(), Some("chosen"));
    }

    #[test]
    fn counts_turns_by_message_and_tools_by_block() {
        let read = |line: &str| ClaudeDialect.read_line(line);
        assert_eq!(
            read(&assistant_line(
                "m1",
                r#"{"type":"text","text":"Reading the law"}"#
            )),
            [AgentEvent::Turn {
                message_id: Some("m1".into())
            }]
        );
        assert_eq!(
            read(&assistant_line(
                "m2",
                r#"{"type":"tool_use","name":"Edit","input":{"file_path":"laws/x/law.yaml","old_string":"a","new_string":"b"}},
                   {"type":"tool_use","name":"Glob","input":{"path":"laws"}}"#,
            )),
            [
                AgentEvent::Turn {
                    message_id: Some("m2".into())
                },
                AgentEvent::Tool("Edit laws/x/law.yaml".into()),
                AgentEvent::Tool("Glob laws".into()),
            ]
        );
    }

    #[test]
    fn other_events_mean_nothing() {
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta"}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#,
            r#"{"type":"system","subtype":"status"}"#,
            "not json",
        ] {
            assert!(ClaudeDialect.read_line(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn an_assistant_event_is_read_whatever_the_key_order() {
        let events = ClaudeDialect.read_line(
            r#"{"message":{"id":"m1","content":[{"type":"tool_use","name":"Read","input":{"file_path":"a"}}]},"type":"assistant"}"#,
        );
        assert_eq!(events[1], AgentEvent::Tool("Read a".into()));
    }

    #[test]
    fn the_closing_event_gives_the_usage_and_the_reason_of_a_failure() {
        assert_eq!(
            ClaudeDialect.read_line(
                r#"{"type":"result","is_error":false,"usage":{"input_tokens":3,"output_tokens":4}}"#
            ),
            [AgentEvent::Closing {
                usage: Some(AgentUsage {
                    input_tokens: 3,
                    output_tokens: 4,
                    ..AgentUsage::default()
                }),
                error: None,
            }]
        );
        assert_eq!(
            ClaudeDialect
                .read_line(r#"{"type":"result","is_error":true,"result":"API Error: 429"}"#),
            [AgentEvent::Closing {
                usage: None,
                error: Some("API Error: 429".into()),
            }]
        );
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
        assert_eq!(
            ClaudeDialect.read_line(init),
            [AgentEvent::Model("claude-opus-5-5".into())]
        );
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
        // `subtype` alone is not enough: the event must be a system event.
        assert_eq!(
            init_model(r#"{"type":"user","subtype":"init","model":"x"}"#),
            None
        );
    }

    #[test]
    fn usage_is_read_from_the_providers_closing_object() {
        // The shape the claude CLI emits under `--output-format json`.
        let tail = r#"{"type":"result","subtype":"success","is_error":false,
            "duration_ms":812345,"num_turns":42,"result":"done",
            "total_cost_usd":1.2345,
            "usage":{"input_tokens":1200,"output_tokens":34567,
                     "cache_read_input_tokens":980000,"cache_creation_input_tokens":45}}"#;
        let u = usage_from_json(tail).expect("usage");
        assert_eq!(u.input_tokens, 1200);
        assert_eq!(u.output_tokens, 34567);
        assert_eq!(u.cache_read_tokens, 980_000);
        // Money as an integer: 1.2345 dollar is 123450 thousandths of a cent.
        assert_eq!(u.cost_millicents, 123_450);
    }

    #[test]
    fn only_the_last_object_in_the_stream_counts() {
        // The stream carries earlier objects; the accounting is the final one.
        let tail = concat!(
            r#"{"type":"assistant","usage":{"input_tokens":1,"output_tokens":1}}"#,
            "\n",
            r#"{"type":"result","usage":{"input_tokens":99,"output_tokens":7}}"#,
        );
        let u = usage_from_json(tail).expect("usage");
        assert_eq!(u.input_tokens, 99);
        assert_eq!(u.output_tokens, 7);
    }

    #[test]
    fn usage_is_found_in_a_stream_whatever_the_key_order() {
        // stream-json as the CLI writes it: the closing event does not start
        // with `type`, and a stream event with a nested `type` follows it.
        let tail = concat!(
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1}}}"#,
            "\n",
            r#"{"duration_api_ms":5,"type":"result","total_cost_usd":0.01,"usage":{"input_tokens":42,"output_tokens":7}}"#,
            "\n",
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
            "\n",
        );
        let u = usage_from_json(tail).expect("usage");
        assert_eq!(u.input_tokens, 42);
        assert_eq!(u.output_tokens, 7);
        assert_eq!(u.cost_millicents, 1_000);
    }

    #[test]
    fn a_truncated_tail_reports_nothing_rather_than_guessing() {
        // The tail is bounded, so a large stream can cut an object in half.
        // Half a figure is worse than no figure.
        assert!(usage_from_json(r#"put_tokens":1200,"output_"#).is_none());
        assert!(usage_from_json("").is_none());
        assert!(usage_from_json("no json here at all").is_none());
    }

    #[test]
    fn an_object_without_usage_reports_nothing() {
        // A provider that reports no accounting must not read as zero cost.
        assert!(usage_from_json(r#"{"type":"result","result":"ok"}"#).is_none());
    }
}
