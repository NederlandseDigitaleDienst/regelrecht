//! Starting an agent CLI and watching it until it ends.
//!
//! Nothing here knows a provider by name: the command line comes from the
//! provider's dialect, and so does the meaning of every line it writes. What
//! is the same for every provider lives here: a stripped environment, its own
//! process group, both pipes drained, and a run that ends on exit, on its
//! timeout, on silence (for a CLI that streams per token) or on the memory
//! ceiling.

use std::sync::Mutex;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use super::dialect::{AgentEvent, CommandRequest, CommandSpec, ToolPolicy};
use crate::enrich::{AgentUsage, EnrichConfig, LlmProvider, SessionAction};
use crate::error::{PipelineError, Result};

/// What one agent subprocess reported about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgentRun {
    /// Token use and cost, from the closing event or the sum of the steps.
    pub usage: Option<AgentUsage>,
    /// The model it ran on: as the CLI named it, or the configured one for a
    /// CLI that names none.
    pub model: Option<String>,
}

/// Max bytes of the LLM subprocess's stderr to retain for diagnostics. The tail
/// (most recent output) is kept and appended to the error on a non-zero exit, so
/// a failure reports the real cause (e.g. an auth `401`) instead of a bare code.
const MAX_STDERR_CAPTURE: usize = 4096;

/// Max bytes of the agent's stdout to retain, enough to hold the provider's
/// closing JSON object with the usage figures and nothing before it.
const MAX_STDOUT_TAIL: usize = 32 * 1024;

/// Allowlisted environment variable prefixes/names that are safe to pass to the
/// LLM subprocess.  Everything else (DATABASE_URL, etc.) is stripped.
const LLM_ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "TERM",
    "LANG",
    "USER",
    "SHELL",
    "TMPDIR",
    "XDG_",
    // Provider-specific auth.
    //
    // NOTE: ANTHROPIC_API_KEY is deliberately NOT forwarded. The claude provider
    // authenticates only with a personal subscription via CLAUDE_CODE_OAUTH_TOKEN.
    // Keeping ANTHROPIC_API_KEY out of the subprocess env means that even if it is
    // still set on the worker (e.g. a leftover), it can never reach `claude` and
    // silently take precedence over the OAuth token — the exact footgun that
    // makes claude fail auth at startup.
    "CLAUDE_CODE_OAUTH_TOKEN",
    "VLAM_API_KEY",
    "OPENCODE_",
];

/// Check whether an environment variable name is on the allowlist.
fn env_allowed(key: &str) -> bool {
    LLM_ENV_ALLOWLIST
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// Turn a dialect's command line into a process.
///
/// The subprocess gets a stripped environment: only variables on
/// `LLM_ENV_ALLOWLIST` are forwarded, then whatever the dialect sets. This
/// prevents leaking DATABASE_URL and other secrets to the LLM process (which
/// may have shell access).
fn spawn_command(spec: &CommandSpec, safe_env: Vec<(String, String)>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&spec.program);
    cmd.env_clear();
    cmd.envs(safe_env);
    cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
    cmd.envs(spec.env.iter().map(|(k, v)| (k, v)));
    cmd.args(&spec.args);
    if let Some(dir) = &spec.current_dir {
        cmd.current_dir(dir);
    }
    // Run the agent as its own process-group leader so a timeout/memory kill can
    // signal the whole tree (the CLI plus any node workers or git it forks), not
    // just the direct child. `kill_on_drop` is a backstop: if the worker future
    // is dropped (panic, early return) the child is reaped rather than orphaned.
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    cmd
}

/// Spawn and supervise an LLM agent subprocess, provider-agnostically.
///
/// The provider's dialect builds the command and reads its output; this drains
/// stdout/stderr (retaining a bounded stderr tail for the error message), and
/// races the child against the configured timeout, the silence check and the
/// RSS memory watchdog, killing the whole process group on any of them. `cwd`
/// is the working directory the agent runs in (and writes its output into);
/// `file_arg` is the optional single input file (OpenCode's `-f`). Callers
/// supply their own `prompt` — enrich and document-convert differ only in that
/// prompt. `tools` says what this call may use (see [`ToolPolicy`]).
/// `session` says whether this call opens, continues or ignores a session; a
/// caller with no window to speak of passes [`SessionAction::Cold`].
pub(crate) async fn run_llm_subprocess(
    provider: &LlmProvider,
    prompt: &str,
    file_arg: Option<&std::path::Path>,
    cwd: &std::path::Path,
    config: &EnrichConfig,
    tools: ToolPolicy<'_>,
    session: SessionAction,
) -> Result<AgentRun> {
    let dialect = provider.dialect();
    let capabilities = dialect.capabilities();
    let provider_name = dialect.name().to_string();
    let configured_model = provider.model();

    let safe_env: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| env_allowed(k)).collect();
    let spec = dialect.command(&CommandRequest {
        program: provider.path(),
        model: configured_model,
        prompt,
        file_arg,
        cwd,
        tools,
        effort: config.effort.as_deref(),
        session,
        env: &safe_env,
        rotation_bucket: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 100)
            .unwrap_or(0),
    });

    // Diagnostic logging: record exactly what will be spawned and which env vars
    // are forwarded (NAMES only — never values), and flag whether
    // ANTHROPIC_API_KEY is still present in the worker env even though it is
    // deliberately never forwarded.
    tracing::info!(
        provider = %provider_name,
        model = ?configured_model,
        effort = ?config.effort,
        prompt_chars = prompt.len(),
        anthropic_api_key_present_in_worker_env = std::env::var_os("ANTHROPIC_API_KEY").is_some(),
        "spawning LLM subprocess"
    );
    // The forwarded env var NAMES are static between spawns — keep them at debug
    // so they don't add a long line to every job's info logs.
    let forwarded_env: Vec<&str> = safe_env.iter().map(|(k, _)| k.as_str()).collect();
    tracing::debug!(provider = %provider_name, forwarded_env = ?forwarded_env, "forwarded env to LLM subprocess");

    let mut cmd = spawn_command(&spec, safe_env);

    // Both streams are piped and drained. stdout is drained-and-discarded: a
    // verbose agent (e.g. opencode `--format json`) inlines the full body of
    // every fetched page into its event stream, which would flood container
    // logs. stderr is drained too — we MUST keep reading both or a full 64 KB
    // OS pipe buffer blocks the child — but for stderr we also keep a bounded
    // tail and re-log previews, so the LLM's real error (e.g. an auth 401) is
    // both visible in the logs and attached to the job's failure.
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| PipelineError::Enrich(format!("failed to spawn {}: {e}", provider_name)))?;

    // Capture the PID before any wait reaps the child; the memory watchdog
    // and process-group kill both need it.
    let pid = child.id();
    let activity = config.activity.clone();
    activity.enter();
    // A CLI that names no model: show the configured one.
    if !capabilities.names_model {
        if let Some(model) = configured_model {
            activity.set_model(model);
        }
    }
    // However this call ends, it stops counting as in flight.
    let _in_flight = ExitOnDrop(activity.clone());
    // Only a CLI that writes per token is stopped for silence. One that writes
    // an event per finished part can be quiet for longer while it writes a
    // large file.
    let idle_limit = if capabilities.streams_per_token {
        config.idle_timeout
    } else {
        None
    };

    // Drain stderr, retaining the last `MAX_STDERR_CAPTURE` bytes for the
    // error message and re-logging bounded previews so it stays visible.
    let stderr_tail = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let stderr_task = child.stderr.take().map(|mut stderr| {
        let tail = stderr_tail.clone();
        let stderr_provider = provider_name.clone();
        let activity = activity.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break, // EOF or pipe gone
                    Ok(n) => {
                        activity.touch();
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let preview: String = text.trim().chars().take(500).collect();
                        if !preview.is_empty() {
                            tracing::warn!(provider = %stderr_provider, %preview, "agent stderr");
                        }
                        if let Ok(mut t) = tail.lock() {
                            t.push_str(&text);
                            if t.len() > MAX_STDERR_CAPTURE {
                                let mut cut = t.len() - MAX_STDERR_CAPTURE;
                                while cut < t.len() && !t.is_char_boundary(cut) {
                                    cut += 1;
                                }
                                *t = t[cut..].to_string();
                            }
                        }
                    }
                }
            }
        })
    });
    // Read the retained stderr tail, formatted as a "; stderr: …" suffix.
    let stderr_suffix = || {
        stderr_tail
            .lock()
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(|t| format!("; stderr: {t}"))
            .unwrap_or_default()
    };

    // Drain the agent's stdout so it never reaches container logs. We MUST
    // keep reading it: if the OS pipe buffer (64 KB) fills, the child blocks
    // indefinitely — the same deadlock the stderr comment above warns about.
    // The task ends on EOF when the process exits or is killed.
    // Tail of stdout, kept for a CLI that reports its usage in one object at
    // the end rather than in an event. Bounded: everything before the last
    // object is of no interest, and the stream can carry megabytes.
    let stdout_tail = std::sync::Arc::new(Mutex::new(String::new()));
    let tail_writer = std::sync::Arc::clone(&stdout_tail);
    // What the dialect read off the stream, for the call's record.
    let report = std::sync::Arc::new(Mutex::new(StreamReport::default()));
    let report_writer = std::sync::Arc::clone(&report);
    let mut stdout_task = None;
    if let Some(mut stdout) = child.stdout.take() {
        let drain_provider = provider_name.clone();
        let activity = activity.clone();
        stdout_task = Some(tokio::spawn(async move {
            let mut lines = crate::agent_activity::LineSplitter::default();
            // Drain in fixed-size chunks rather than whole lines: opencode
            // inlines multi-MB page bodies as a single JSON line, so a
            // line reader would allocate the entire body just to log a
            // 200-char preview — a heap spike on the very worker this
            // watchdog exists to protect. Reading into a fixed buffer keeps
            // the pipe empty without ever holding more than `buf`.
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) | Err(_) => break, // EOF or pipe gone (process exited/killed)
                    Ok(n) => {
                        activity.touch();
                        lines.feed(&buf[..n], |line| {
                            for event in dialect.read_line(line) {
                                activity.record(&event);
                                if let Ok(mut r) = report_writer.lock() {
                                    r.take(&drain_provider, event);
                                }
                            }
                        });
                        // Bounded preview at debug only (off under the
                        // default "info" subscriber). The leading bytes of a
                        // read carry the event type and ids, not the large
                        // inlined bodies; lossy is fine for a log preview.
                        let preview = String::from_utf8_lossy(&buf[..n.min(200)]);
                        let preview = preview.trim_end();
                        if !preview.is_empty() {
                            tracing::debug!(provider = %drain_provider, %preview, "agent stdout");
                        }
                        #[allow(clippy::unwrap_used)]
                        if let Ok(mut tail) = tail_writer.lock() {
                            tail.push_str(&String::from_utf8_lossy(&buf[..n]));
                            if tail.len() > MAX_STDOUT_TAIL {
                                let cut = tail.len() - MAX_STDOUT_TAIL;
                                let cut = tail
                                    .char_indices()
                                    .map(|(i, _)| i)
                                    .find(|i| *i >= cut)
                                    .unwrap_or(tail.len());
                                *tail = tail.split_off(cut);
                            }
                        }
                    }
                }
            }
        }));
    }

    let status = tokio::select! {
        result = child.wait() => {
            result.map_err(|e| {
                PipelineError::Enrich(format!("failed to wait for {}: {e}", provider_name))
            })?
        }
        _ = tokio::time::sleep(config.timeout) => {
            terminate(&mut child, pid).await;
            // Abort the drain task rather than leaving it detached: if a
            // grandchild inherited fd 2 and survived terminate(), the task
            // would otherwise leak. The tail read below is already populated.
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{} timed out after {:?}{}{}",
                provider_name,
                config.timeout,
                stderr_suffix(),
                activity.summary()
            )));
        }
        silent = wait_for_silence(&activity, idle_limit) => {
            tracing::error!(
                provider = %provider_name,
                silent_secs = silent.as_secs(),
                "LLM subprocess wrote nothing for too long, killing it as stalled"
            );
            terminate(&mut child, pid).await;
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{} stalled: no output for {}s{}{}",
                provider_name,
                silent.as_secs(),
                stderr_suffix(),
                activity.summary()
            )));
        }
        observed_mb = watch_memory(pid, config.max_rss_mb) => {
            tracing::error!(
                provider = %provider_name,
                pid = ?pid,
                observed_mb,
                limit_mb = config.max_rss_mb,
                "LLM subprocess exceeded memory limit, killing to protect the container"
            );
            terminate(&mut child, pid).await;
            if let Some(t) = &stderr_task {
                t.abort();
            }
            return Err(PipelineError::Enrich(format!(
                "{provider_name} exceeded memory limit of {} MB (RSS {observed_mb} MB), killed{}",
                config.max_rss_mb, stderr_suffix()
            )));
        }
    };

    // Let stdout drain to its end so the closing event is in, on success for
    // its usage and on failure for its error text. Bounded: a grandchild
    // that inherited the pipe could keep it open and never EOF, and this
    // await sits outside the timeout/memory select!.
    if let Some(task) = stdout_task {
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }
    let report = report.lock().map(|r| r.clone()).unwrap_or_default();

    if !status.success() {
        // Give the stderr drain a moment to finish so the tail is complete,
        // bounded for the same reason as stdout above. Best-effort, like the
        // tail in the other paths.
        if let Some(task) = stderr_task {
            let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
        }
        // A CLI can report a failed API call in its closing event and exit 1
        // with nothing on stderr; without this the reason is lost.
        let cli_error = report
            .closing_error
            .as_deref()
            .map(|t| format!(": {t}"))
            .unwrap_or_default();
        return Err(PipelineError::Enrich(format!(
            "{} exited with {}{}{}{}",
            provider_name,
            status,
            cli_error,
            stderr_suffix(),
            activity.summary()
        )));
    }

    // Success: abort the drain task instead of leaving it detached — same
    // fd-2/grandchild-leak guard as the timeout/OOM paths. Normally it has
    // already finished (the child closed stderr on exit); aborting a finished
    // task is a no-op.
    if let Some(t) = &stderr_task {
        t.abort();
    }

    let usage = report.closing_usage.or(report.step_usage).or_else(|| {
        stdout_tail
            .lock()
            .ok()
            .and_then(|tail| dialect.usage_from_tail(&tail))
    });
    if let Some(u) = usage {
        tracing::info!(
            provider = %provider_name,
            resumed = matches!(session, SessionAction::Resume(_)),
            input_tokens = u.input_tokens,
            output_tokens = u.output_tokens,
            cache_read_tokens = u.cache_read_tokens,
            cost_millicents = u.cost_millicents,
            "agent run accounted"
        );
    }
    // A CLI that names no model ran on the one it was told to use; that is
    // the best record there is.
    let model = report.model.or_else(|| {
        (!capabilities.names_model)
            .then(|| configured_model.map(str::to_string))
            .flatten()
    });
    Ok(AgentRun { usage, model })
}

/// What the dialect read off one call's stdout.
#[derive(Debug, Clone, Default)]
struct StreamReport {
    /// The model the CLI named.
    model: Option<String>,
    /// The CLI's own total, from its closing event.
    closing_usage: Option<AgentUsage>,
    /// The sum of the steps, for a CLI with no closing total.
    step_usage: Option<AgentUsage>,
    /// Why the call failed, as the closing event said.
    closing_error: Option<String>,
}

impl StreamReport {
    fn take(&mut self, provider: &str, event: AgentEvent) {
        match event {
            AgentEvent::Model(model) => {
                tracing::info!(%provider, %model, "agent session started");
                self.model = Some(model);
            }
            AgentEvent::Closing { usage, error } => {
                self.closing_usage = usage;
                self.closing_error = error;
            }
            AgentEvent::StepUsage(step) => {
                self.step_usage = Some(match self.step_usage.take() {
                    Some(sum) => sum.plus(step),
                    None => step,
                });
            }
            // stdout is not logged, so an error event is said here, bounded.
            AgentEvent::ErrorEvent(preview) => {
                tracing::warn!(%provider, event = %preview, "agent reported an error");
            }
            AgentEvent::Turn { .. } | AgentEvent::Tool(_) | AgentEvent::StepFinished(_) => {}
        }
    }
}

/// Marks the agent subprocess as ended when its call returns, by any path.
struct ExitOnDrop(crate::agent_activity::ActivityHandle);

impl Drop for ExitOnDrop {
    fn drop(&mut self) {
        self.0.exit();
    }
}

/// Resolve once the agent has written nothing for `limit`, with how long it
/// has been silent. Never resolves when the check is off.
async fn wait_for_silence(
    activity: &crate::agent_activity::ActivityHandle,
    limit: Option<Duration>,
) -> Duration {
    let Some(limit) = limit else {
        return std::future::pending().await;
    };
    // Check a few times per limit, at most every 15 s, so a short limit in a
    // test still fires promptly.
    let every = (limit / 4).clamp(Duration::from_millis(50), Duration::from_secs(15));
    loop {
        tokio::time::sleep(every).await;
        if let Some(silent) = activity.idle() {
            if silent >= limit {
                return silent;
            }
        }
    }
}

/// Kill the LLM subprocess and reap it.
///
/// Signals the whole process group (negative pid) so any helpers the agent
/// forked (node workers, git) die too — not just the direct child — then
/// falls back to `child.kill()` (covers a missing pid) and waits to avoid a
/// zombie.
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    kill_process_group(pid);
    if let Err(e) = child.kill().await {
        // After the group SIGKILL above the direct child is usually already
        // gone, so `ESRCH` here is the expected benign race — only a different
        // error is worth flagging.
        if e.raw_os_error() != Some(libc::ESRCH) {
            tracing::warn!(error = %e, "failed to kill LLM process");
        }
    }
    let _ = child.wait().await;
}

/// Send `SIGKILL` to the entire process group led by `pid`.
///
/// The subprocess is spawned as its own process-group leader
/// (`process_group(0)` in `spawn_command`), so `kill(-pid, …)` reaps the agent
/// and everything it forked. A failure (e.g. `ESRCH`) just means the group has
/// already exited and is ignored.
fn kill_process_group(pid: Option<u32>) {
    if let Some(pid) = pid {
        // Guard against pid 0: `kill(-0, …)` collapses to `kill(0, …)`, which
        // POSIX routes to the *caller's own* process group — it would SIGKILL
        // the worker itself. `child.id()` never yields 0 in practice, but the
        // consequence is catastrophic enough to refuse it defensively.
        if pid == 0 {
            return;
        }
        // SAFETY: `kill(2)` with a negative pid targets a process group and has
        // no memory-safety implications; the return value is intentionally
        // ignored (the group may already be gone). Linux PIDs are capped well
        // below `i32::MAX` (`pid_max` <= 2^22), so `pid as i32` cannot overflow.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

/// Memory watchdog: resolve with the observed RSS (in MB) once the subprocess
/// exceeds `max_rss_mb`.
///
/// The LLM agent accumulates fetched-page bodies in memory across a run; left
/// unchecked it climbs to the container's cgroup limit and triggers an OOM kill
/// of the whole pod. Polling RSS and killing the offender instead turns that
/// into a clean, retryable job failure.
///
/// Disabled (never resolves, letting the `child.wait()`/timeout branches win)
/// when `pid` is `None` or `max_rss_mb` is 0. We poll `/proc` directly rather
/// than capping virtual memory with `RLIMIT_AS`: V8 reserves a huge virtual
/// address space, so a virtual-memory ceiling would crash Node outright.
async fn watch_memory(pid: Option<u32>, max_rss_mb: u64) -> u64 {
    let pid = match pid {
        Some(p) if max_rss_mb > 0 => p,
        _ => {
            tracing::debug!("enrich memory watchdog disabled (no pid or zero limit)");
            std::future::pending::<()>().await;
            unreachable!("pending future never resolves");
        }
    };

    // RSS polling reads `/proc/<pid>/status`, which only exists on Linux (the
    // deploy target). On a non-Linux dev machine `read_vmrss_kb` would return
    // `None` every poll and the watchdog would never fire — silently. Make that
    // degradation explicit and skip the pointless poll loop rather than letting
    // a developer believe a ceiling is enforced when it is not.
    if !cfg!(target_os = "linux") {
        tracing::debug!(
            "enrich memory watchdog inactive: RSS polling needs /proc (Linux only); \
             agent runs without a memory ceiling on this platform"
        );
        std::future::pending::<()>().await;
        unreachable!("pending future never resolves");
    }

    let interval = Duration::from_secs(5);
    loop {
        tokio::time::sleep(interval).await;
        // A missing/unparsable status file means the process is gone; keep
        // looping (harmless) and let `child.wait()` win the select.
        if let Some(rss_kb) = read_vmrss_kb(pid).await {
            let rss_mb = rss_kb / 1024;
            if rss_mb > max_rss_mb {
                return rss_mb;
            }
        }
    }
}

/// Read a process's resident set size (RSS) from `/proc/<pid>/status`, in kB.
/// Returns `None` if the file is missing or unparsable (e.g. the process exited).
///
/// This measures only the direct child PID, not the whole process group that
/// `kill_process_group` SIGKILLs. That asymmetry is intentional: for the current
/// opencode/Claude agent topology the runaway memory is the accumulated
/// fetched-page bodies held in the main node process we spawn, so its RSS is the
/// signal that matters. If a future agent moved that growth into a forked
/// subprocess, this would need to sum the group's RSS (e.g. walk
/// `/proc/<pid>/task` / children) to stay accurate.
async fn read_vmrss_kb(pid: u32) -> Option<u64> {
    let status = tokio::fs::read_to_string(format!("/proc/{pid}/status"))
        .await
        .ok()?;
    parse_vmrss_kb(&status)
}

/// Parse the `VmRSS` value (in kB) out of `/proc/<pid>/status` contents.
fn parse_vmrss_kb(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb| kb.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vmrss_kb_extracts_value() {
        let status = "Name:\tnode\nVmPeak:\t 4194304 kB\nVmRSS:\t  2097152 kB\nThreads:\t12\n";
        assert_eq!(parse_vmrss_kb(status), Some(2_097_152));
    }

    #[test]
    fn test_parse_vmrss_kb_missing_or_malformed() {
        // No VmRSS line.
        assert_eq!(parse_vmrss_kb("Name:\tnode\nThreads:\t12\n"), None);
        // VmRSS present but value not numeric.
        assert_eq!(parse_vmrss_kb("VmRSS:\t  notanumber kB\n"), None);
        // Empty input.
        assert_eq!(parse_vmrss_kb(""), None);
    }

    #[test]
    fn only_allowlisted_variables_reach_the_agent() {
        for allowed in [
            "PATH",
            "HOME",
            "XDG_CONFIG_HOME",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "OPENCODE_MODEL",
            "VLAM_API_KEY",
        ] {
            assert!(env_allowed(allowed), "{allowed}");
        }
        for withheld in ["DATABASE_URL", "ANTHROPIC_API_KEY", "CORPUS_GIT_TOKEN"] {
            assert!(!env_allowed(withheld), "{withheld}");
        }
    }
}
