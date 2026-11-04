//! The opencode CLI, run with `--format json`, talking to VLAM.
//!
//! It writes one JSON event per finished part rather than per token:
//! `step_start` opens a model step, `tool_use` reports a finished tool call,
//! `step_finish` says why a step ended and what it used, and `error` reports a
//! failed model call. It names no model and writes no closing total, and it
//! cannot continue a session.
//!
//! The worker also prepares opencode's own configuration at startup, which
//! the image's entrypoint used to do in shell: see [`prepare_worker`].

use std::path::Path;

use super::dialect::{
    AgentDialect, AgentEvent, Capabilities, CommandRequest, CommandSpec, StepFinish,
};
use crate::enrich::{AgentUsage, LlmProvider, SessionAction};
use crate::error::{PipelineError, Result};

/// Where the image keeps opencode's provider configuration
/// (`packages/pipeline/opencode.json`, copied in by the Dockerfile).
pub(crate) const IMAGE_CONFIG: &str = "/opt/opencode/opencode.json";

/// The longest error event kept for the log. The event is small; the bodies
/// around it in the stream are not.
const MAX_ERROR_PREVIEW_CHARS: usize = 1000;

pub(crate) struct OpenCodeDialect;

impl AgentDialect for OpenCodeDialect {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            resumes_sessions: false,
            // An event per finished part: it can be quiet for a long time
            // while it writes a large file.
            streams_per_token: false,
            names_model: false,
        }
    }

    fn command(&self, req: &CommandRequest<'_>) -> CommandSpec {
        let mut cmd = CommandSpec::new(req.program);
        cmd.arg("run").arg(req.prompt);
        if let Some(f) = req.file_arg {
            cmd.arg("-f").arg(f);
        }
        cmd.arg("--format").arg("json").arg("--dir").arg(req.cwd);
        if let Some(m) = req.model {
            cmd.arg("-m").arg(m);
        }
        // OpenCode has no effort flag; saying so beats passing an option
        // it would reject and letting the run fail on a typo in config.
        if req.effort.is_some() {
            tracing::warn!("effort is not supported by opencode; ignored");
        }
        // Session reuse is a claude-provider affair. Under opencode every
        // call stays cold, and it says so rather than letting a run be
        // read as though the windows had been shared.
        if !matches!(req.session, SessionAction::Cold) {
            tracing::warn!("session reuse is not wired for opencode; this call runs cold");
        }
        cmd
    }

    fn read_line(&self, line: &str) -> Vec<AgentEvent> {
        // Lines can be a megabyte of inlined file content; only parse the ones
        // that can be an event this reads.
        if line.contains(r#""type":"step_start""#)
            || line.contains(r#""type":"tool_use""#)
            || line.contains(r#""type":"step_finish""#)
        {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                match event.get("type").and_then(serde_json::Value::as_str) {
                    Some("step_start") => return vec![AgentEvent::Turn { message_id: None }],
                    Some("tool_use") => {
                        return opencode_tool_summary(&event)
                            .map(AgentEvent::Tool)
                            .into_iter()
                            .collect();
                    }
                    Some("step_finish") => {
                        let mut out = vec![AgentEvent::StepFinished(step_finish_fields(&event))];
                        out.extend(step_usage(&event).map(AgentEvent::StepUsage));
                        return out;
                    }
                    _ => {}
                }
            }
        }
        if is_error_event(line) {
            let preview: String = line.chars().take(MAX_ERROR_PREVIEW_CHARS).collect();
            return vec![AgentEvent::ErrorEvent(preview)];
        }
        Vec::new()
    }
}

/// The opencode provider as `from_env` builds it: `OPENCODE_PATH` or
/// `LLM_PATH` for the binary, `OPENCODE_MODEL` or `LLM_MODEL` for the model.
pub(crate) fn provider_from_env(var: impl Fn(&str) -> Option<String>) -> LlmProvider {
    LlmProvider::OpenCode {
        path: var("OPENCODE_PATH")
            .or_else(|| var("LLM_PATH"))
            .unwrap_or_else(|| "opencode".into())
            .into(),
        model: var("OPENCODE_MODEL").or_else(|| var("LLM_MODEL")),
    }
}

/// What preparing opencode at worker startup found.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkerSetup {
    /// The model to record for opencode calls when the environment names
    /// none: the one in the image's configuration, which is what opencode
    /// runs on. opencode names no model in its own output.
    pub model: Option<String>,
}

/// Prepare opencode for the enrich worker: its credentials, its endpoint, and
/// the model to record. The image's entrypoint did this in shell before the
/// worker started.
///
/// - With `VLAM_API_KEY` set, write it to opencode's `auth.json`, readable
///   by the owner only.
/// - Write opencode's user configuration: the image's, with the endpoint
///   pointed at `$VLAM_API_URL/v1`. The only route to VLAM is the ZAD `vlam`
///   service, which sets that variable; without it the provider is left with
///   no endpoint rather than pointed at a public one, and a warning says so.
/// - When neither `OPENCODE_MODEL` nor `LLM_MODEL` names a model, report the
///   one in the image's configuration.
///
/// Does nothing outside the image (no configuration at `image_config`), so a
/// worker started on a developer's machine never writes into their home. Only
/// the enrich worker calls this; `enrich-once` uses the developer's own
/// opencode configuration.
pub(crate) fn prepare_worker(
    home: Option<&Path>,
    image_config: &Path,
    var: impl Fn(&str) -> Option<String>,
) -> Result<WorkerSetup> {
    let var = |name: &str| var(name).filter(|v| !v.is_empty());
    if !image_config.exists() {
        tracing::info!(
            config = %image_config.display(),
            "no opencode configuration from the image; leaving opencode's setup alone"
        );
        return Ok(WorkerSetup::default());
    }
    let home = home.ok_or_else(|| {
        PipelineError::Config("HOME is not set; opencode cannot be prepared".into())
    })?;
    let mut config: serde_json::Value = std::fs::read_to_string(image_config)
        .map_err(|e| setup_error(image_config, e))
        .and_then(|text| serde_json::from_str(&text).map_err(|e| setup_error(image_config, e)))?;

    if let Some(key) = var("VLAM_API_KEY") {
        let auth = home.join(".local/share/opencode/auth.json");
        let body = serde_json::json!({"vlam": {"type": "api", "key": key}});
        write_private(&auth, &body.to_string())?;
    }

    // As the shell's `${VLAM_API_URL%/}/v1`: one trailing slash dropped.
    let base_url = var("VLAM_API_URL").map(|url| {
        let url = url.strip_suffix('/').unwrap_or(&url);
        format!("{url}/v1")
    });
    match &base_url {
        Some(url) => {
            let vlam = config
                .get_mut("provider")
                .and_then(|p| p.get_mut("vlam"))
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    PipelineError::Config(format!(
                        "{} has no provider.vlam to point at VLAM_API_URL",
                        image_config.display()
                    ))
                })?;
            vlam.insert("options".into(), serde_json::json!({"baseURL": url}));
        }
        None => tracing::warn!(
            "VLAM_API_URL is not set (no ZAD vlam service); the opencode provider has no endpoint"
        ),
    }
    let user_config = home.join(".config/opencode/opencode.json");
    let pretty = serde_json::to_string_pretty(&config)
        .map_err(|e| PipelineError::Config(format!("opencode configuration: {e}")))?;
    write_file(&user_config, &pretty)?;

    let model = if var("OPENCODE_MODEL").is_none() && var("LLM_MODEL").is_none() {
        config
            .get("model")
            .and_then(serde_json::Value::as_str)
            .filter(|m| !m.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    tracing::info!(
        endpoint = base_url.is_some(),
        key = var("VLAM_API_KEY").is_some(),
        model = ?model,
        "opencode prepared"
    );
    Ok(WorkerSetup { model })
}

/// Give an opencode provider `model` when it names none (or an empty one).
/// Any other provider is left alone.
pub(crate) fn fill_default_model(provider: &mut LlmProvider, model: &str) {
    if let LlmProvider::OpenCode { model: m, .. } = provider {
        if m.as_deref().is_none_or(str::is_empty) {
            *m = Some(model.to_string());
        }
    }
}

fn setup_error(path: &Path, e: impl std::fmt::Display) -> PipelineError {
    PipelineError::Config(format!("opencode setup, {}: {e}", path.display()))
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| setup_error(dir, e))?;
    }
    std::fs::write(path, contents).map_err(|e| setup_error(path, e))
}

/// Write a file only its owner can read, for a credential.
fn write_private(path: &Path, contents: &str) -> Result<()> {
    write_file(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| setup_error(path, e))?;
    }
    Ok(())
}

/// Whether a JSON line from the agent is an error event (`"type":"error"`),
/// as opencode writes when a model call fails.
fn is_error_event(line: &str) -> bool {
    // Lines can be a megabyte of inlined file content; only parse the ones
    // that can be an error event.
    if !line.contains(r#""type":"error""#) {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|t| t == "error"))
        .unwrap_or(false)
}

/// The usage of one model step, from its `step_finish` event. opencode has no
/// closing event with a total, so a call's usage is the sum of its steps.
/// Reasoning tokens are billed as output, so they count as output here.
/// `None` for an event without a `part`.
fn step_usage(event: &serde_json::Value) -> Option<AgentUsage> {
    let part = event.get("part")?;
    let tokens = part.get("tokens");
    let n = |path: &[&str]| -> u64 {
        let mut value = match tokens {
            Some(t) => t,
            None => return 0,
        };
        for key in path {
            value = match value.get(*key) {
                Some(v) => v,
                None => return 0,
            };
        }
        value.as_u64().unwrap_or(0)
    };
    Some(AgentUsage {
        input_tokens: n(&["input"]),
        output_tokens: n(&["output"]) + n(&["reasoning"]),
        cache_read_tokens: n(&["cache", "read"]),
        cache_write_tokens: n(&["cache", "write"]),
        cost_millicents: part
            .get("cost")
            .and_then(serde_json::Value::as_f64)
            .map_or(0, |usd| (usd * 100_000.0).round() as u64),
    })
}

/// Read a `step_finish` event; a missing field reads as `unknown` or 0.
fn step_finish_fields(event: &serde_json::Value) -> StepFinish {
    let part = event.get("part");
    let reason = part
        .and_then(|p| p.get("reason"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let tokens = part.and_then(|p| p.get("tokens"));
    let count = |key: &str| {
        tokens
            .and_then(|t| t.get(key))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    StepFinish {
        reason,
        input: count("input"),
        output: count("output"),
        reasoning: count("reasoning"),
    }
}

/// `name target (status)` for a `tool_use` event. The target is the file path
/// only, as in claude's tool summary: never the content of a write, a search
/// pattern or a shell command.
fn opencode_tool_summary(event: &serde_json::Value) -> Option<String> {
    let part = event.get("part")?;
    let name = part.get("tool")?.as_str()?;
    let state = part.get("state");
    let status = state
        .and_then(|s| s.get("status"))
        .and_then(serde_json::Value::as_str);
    let target = state
        .and_then(|s| s.get("input"))
        .and_then(|i| i.get("filePath"))
        .and_then(serde_json::Value::as_str);
    let mut out = name.to_string();
    if let Some(t) = target {
        out.push(' ');
        out.push_str(t);
    }
    if let Some(st) = status.filter(|st| *st != "completed") {
        out.push_str(&format!(" ({st})"));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::agent::dialect::ToolPolicy;

    fn read(line: &str) -> Vec<AgentEvent> {
        OpenCodeDialect.read_line(line)
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    #[test]
    fn the_command_runs_json_in_the_directory_with_the_model() {
        let cmd = OpenCodeDialect.command(&CommandRequest {
            program: Path::new("opencode"),
            model: Some("vlam/kimi-k3-prepaid"),
            prompt: "prompt",
            file_arg: Some(Path::new("/work/law.yaml")),
            cwd: Path::new("/work"),
            tools: ToolPolicy {
                allow_bash: false,
                deny: &[],
            },
            effort: Some("high"),
            session: SessionAction::Cold,
            env: &[],
            rotation_bucket: 0,
        });
        assert_eq!(
            cmd.args_lossy(),
            [
                "run",
                "prompt",
                "-f",
                "/work/law.yaml",
                "--format",
                "json",
                "--dir",
                "/work",
                "-m",
                "vlam/kimi-k3-prepaid"
            ]
        );
        // opencode takes its directory as a flag, not as the process's.
        assert_eq!(cmd.current_dir, None);
        assert!(cmd.env.is_empty());
    }

    #[test]
    fn opencode_steps_and_tool_calls_are_read() {
        assert_eq!(
            read(r#"{"type":"step_start","part":{"type":"step-start"}}"#),
            [AgentEvent::Turn { message_id: None }]
        );
        assert_eq!(
            read(
                r#"{"type":"tool_use","part":{"type":"tool","tool":"edit","state":{"status":"completed","input":{"filePath":"law.yaml","oldString":"secret"}}}}"#
            ),
            [AgentEvent::Tool("edit law.yaml".into())]
        );
        assert_eq!(
            read(
                r#"{"type":"tool_use","part":{"type":"tool","tool":"write","state":{"status":"error"}}}"#
            ),
            [AgentEvent::Tool("write (error)".into())]
        );
    }

    #[test]
    fn step_finish_fields_reads_reason_and_tokens() {
        let event: serde_json::Value = serde_json::from_str(
            r#"{"type":"step_finish","part":{"reason":"length","tokens":{"input":50000,"output":16384,"reasoning":12000,"cache":{"read":0}}}}"#,
        )
        .unwrap();
        assert_eq!(
            step_finish_fields(&event),
            StepFinish {
                reason: "length".into(),
                input: 50000,
                output: 16384,
                reasoning: 12000
            }
        );
        let bare: serde_json::Value = serde_json::from_str(r#"{"type":"step_finish"}"#).unwrap();
        assert_eq!(
            step_finish_fields(&bare),
            StepFinish {
                reason: "unknown".into(),
                input: 0,
                output: 0,
                reasoning: 0
            }
        );
        // Without a part a step still ends, but reports no usage.
        assert_eq!(
            read(r#"{"type":"step_finish"}"#),
            [AgentEvent::StepFinished(StepFinish {
                reason: "unknown".into(),
                input: 0,
                output: 0,
                reasoning: 0
            })]
        );
    }

    #[test]
    fn opencode_step_usage_counts_reasoning_as_output_and_reads_cost() {
        let line = r#"{"type":"step_finish","part":{"reason":"tool-calls","cost":0.0123,"tokens":{"input":40000,"output":900,"reasoning":5100,"cache":{"read":30000,"write":10}}}}"#;
        assert_eq!(
            read(line)[1],
            AgentEvent::StepUsage(AgentUsage {
                input_tokens: 40000,
                output_tokens: 6000,
                cache_read_tokens: 30000,
                cache_write_tokens: 10,
                cost_millicents: 1230,
            })
        );
        // Other events are not steps.
        assert_eq!(read(r#"{"type":"step_start","part":{}}"#).len(), 1);
        // Valid JSON that contains the pair, nested: it passes the cheap
        // substring check, so only the top-level `type` check rejects it.
        let nested = r#"{"type":"tool_use","part":{"state":{"output":{"type":"step_finish","tokens":{"input":5}}}}}"#;
        assert!(serde_json::from_str::<serde_json::Value>(nested).is_ok());
        assert!(nested.contains(r#""type":"step_finish""#));
        assert!(!read(nested)
            .iter()
            .any(|e| matches!(e, AgentEvent::StepUsage(_))));
    }

    #[test]
    fn is_error_event_spots_an_opencode_error_event() {
        assert!(is_error_event(
            r#"{"type":"error","error":{"name":"APIError","data":{"message":"401"}}}"#
        ));
        assert!(!is_error_event(
            r#"{"type":"text","part":{"text":"error"}}"#
        ));
        assert!(!is_error_event("not json, mentions \"type\":\"error\""));
        assert!(matches!(
            read(r#"{"type":"error","error":{"name":"APIError"}}"#)[..],
            [AgentEvent::ErrorEvent(_)]
        ));
    }

    #[test]
    fn the_provider_is_read_from_the_environment() {
        let LlmProvider::OpenCode { path, model } = provider_from_env(env(&[])) else {
            panic!("an opencode provider")
        };
        assert_eq!(path, Path::new("opencode"));
        assert_eq!(model, None);
        let LlmProvider::OpenCode { path, model } = provider_from_env(env(&[
            ("LLM_PATH", "/bin/llm"),
            ("OPENCODE_PATH", "/bin/opencode"),
            ("LLM_MODEL", "fallback"),
        ])) else {
            panic!("an opencode provider")
        };
        assert_eq!(path, Path::new("/bin/opencode"));
        assert_eq!(model.as_deref(), Some("fallback"));
    }

    const IMAGE: &str =
        r#"{"model":"vlam/kimi-k3-prepaid","provider":{"vlam":{"name":"VLAM","models":{}}}}"#;

    fn image(dir: &Path) -> PathBuf {
        let path = dir.join("image/opencode.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, IMAGE).unwrap();
        path
    }

    fn written(path: PathBuf) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn the_worker_writes_the_key_the_endpoint_and_reports_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let setup = prepare_worker(
            Some(&home),
            &image(dir.path()),
            env(&[
                ("VLAM_API_KEY", r#"key "with" quotes\"#),
                ("VLAM_API_URL", "https://vlam.example/"),
            ]),
        )
        .unwrap();
        assert_eq!(setup.model.as_deref(), Some("vlam/kimi-k3-prepaid"));

        let auth_path = home.join(".local/share/opencode/auth.json");
        assert_eq!(
            written(auth_path.clone()),
            serde_json::json!({"vlam": {"type": "api", "key": r#"key "with" quotes\"#}})
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&auth_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        let config = written(home.join(".config/opencode/opencode.json"));
        assert_eq!(
            config["provider"]["vlam"]["options"],
            serde_json::json!({"baseURL": "https://vlam.example/v1"})
        );
        // The rest of the image's configuration is kept.
        assert_eq!(config["provider"]["vlam"]["name"], "VLAM");
        assert_eq!(config["model"], "vlam/kimi-k3-prepaid");
    }

    #[test]
    fn without_an_endpoint_or_key_the_config_is_written_as_the_image_has_it() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        // An empty value counts as unset, as `-n` / `-z` did in the shell.
        prepare_worker(
            Some(&home),
            &image(dir.path()),
            env(&[("VLAM_API_KEY", "")]),
        )
        .unwrap();
        assert!(!home.join(".local/share/opencode/auth.json").exists());
        let config = written(home.join(".config/opencode/opencode.json"));
        assert_eq!(
            config,
            serde_json::from_str::<serde_json::Value>(IMAGE).unwrap()
        );
    }

    #[test]
    fn a_model_named_in_the_environment_is_left_to_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        for name in ["OPENCODE_MODEL", "LLM_MODEL"] {
            let setup =
                prepare_worker(Some(&home), &image(dir.path()), env(&[(name, "other")])).unwrap();
            assert_eq!(setup.model, None, "{name}");
        }
    }

    #[test]
    fn outside_the_image_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let setup = prepare_worker(
            Some(&home),
            &dir.path().join("missing/opencode.json"),
            env(&[
                ("VLAM_API_KEY", "key"),
                ("VLAM_API_URL", "https://vlam.example"),
            ]),
        )
        .unwrap();
        assert_eq!(setup, WorkerSetup::default());
        assert!(!home.exists());
    }

    #[test]
    fn a_broken_image_config_stops_the_worker() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("opencode.json");
        std::fs::write(&config, "{not json").unwrap();
        let err = prepare_worker(Some(dir.path()), &config, env(&[]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("opencode setup"), "{err}");
        // An endpoint with nowhere to go in the config is an error too.
        std::fs::write(&config, r#"{"model":"m"}"#).unwrap();
        let err = prepare_worker(
            Some(dir.path()),
            &config,
            env(&[("VLAM_API_URL", "https://vlam.example")]),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("has no provider.vlam"), "{err}");
        // Outside the image a missing HOME does not matter; inside it does.
        assert!(prepare_worker(None, &dir.path().join("missing.json"), env(&[])).is_ok());
        std::fs::write(&config, IMAGE).unwrap();
        assert!(prepare_worker(None, &config, env(&[])).is_err());
    }
}
