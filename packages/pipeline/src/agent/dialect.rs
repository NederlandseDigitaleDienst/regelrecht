//! What a provider's CLI is, as far as the supervisor needs to know: how to
//! start it, what it can do, and what its output lines mean.
//!
//! A dialect does no I/O. It turns a request into a command line and a line of
//! stdout into [`AgentEvent`]s, so each provider's quirks can be tested with
//! strings in and values out. Starting the process, reading its pipes, timing
//! it out and killing it is the supervisor's work and the same for every
//! provider.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::enrich::{AgentUsage, SessionAction};

/// What tools one agent call may use.
///
/// Two fields because the provider takes two answers and they are not the same
/// question. The allowlist decides what is approved without asking; the deny
/// list decides what is not there at all. Conflating them is how the enrichment
/// lane came to report a shell-less runtime to an agent that had a shell.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ToolPolicy<'a> {
    /// Whether the `claude` provider's allowlist includes `Bash` (enrich keeps
    /// it off; document-convert needs it so the agent can run/install a
    /// converter). No effect on `opencode`, which has its own tool model.
    pub allow_bash: bool,
    /// Tools withheld outright. Unlike leaving a tool off the allowlist, which
    /// only means it has to be asked for, this takes it away.
    pub deny: &'a [String],
}

/// What a provider can do, which decides how the worker treats it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Capabilities {
    /// Whether a call can continue an earlier call's session. Without it every
    /// call runs cold, whatever `ENRICH_SESSION_REUSE` asks for.
    pub resumes_sessions: bool,
    /// Whether the CLI writes while the model generates, rather than once per
    /// finished part. Only then does a silence mean the agent has stopped.
    pub streams_per_token: bool,
    /// Whether the CLI names the model it runs on. Without it the worker
    /// records the model it asked for.
    pub names_model: bool,
}

/// Everything a dialect needs to build one command line.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandRequest<'a> {
    pub program: &'a Path,
    /// The configured model, if any.
    pub model: Option<&'a str>,
    pub prompt: &'a str,
    /// The single input file, for a CLI that takes one (opencode's `-f`).
    pub file_arg: Option<&'a Path>,
    /// Where the agent works and writes.
    pub cwd: &'a Path,
    pub tools: ToolPolicy<'a>,
    pub effort: Option<&'a str>,
    pub session: SessionAction,
    /// The environment the subprocess will get, already filtered to the
    /// allowlist. A dialect reads its credentials from here, never from the
    /// worker's own environment.
    pub env: &'a [(String, String)],
    /// A number that changes every ~100 seconds, for spreading load over
    /// several credentials. Passed in so the dialect needs no clock.
    pub rotation_bucket: u64,
}

/// A command line, as data. The supervisor turns it into a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Set after the allowlisted environment, so these win.
    pub env: Vec<(String, String)>,
    /// The process's working directory, for a CLI that takes it that way
    /// rather than as a flag.
    pub current_dir: Option<PathBuf>,
}

impl CommandSpec {
    pub(crate) fn new(program: &Path) -> Self {
        Self {
            program: program.to_path_buf(),
            args: Vec::new(),
            env: Vec::new(),
            current_dir: None,
        }
    }

    pub(crate) fn arg(&mut self, arg: impl Into<OsString>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    /// The arguments as strings, for tests and logs.
    #[cfg(test)]
    pub(crate) fn args_lossy(&self) -> Vec<String> {
        self.args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }
}

/// What an opencode `step_finish` event says about the step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepFinish {
    pub reason: String,
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
}

/// What one line of a provider's stdout means, in terms every provider shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentEvent {
    /// The CLI named the model it runs on.
    Model(String),
    /// A model turn. Turns that carry a message id are counted once per id,
    /// since the CLI may write one event per content block of a message.
    Turn { message_id: Option<String> },
    /// A tool call, as a summary that carries no content (see `tool_summary`).
    Tool(String),
    /// A model step ended, with why and what it used.
    StepFinished(StepFinish),
    /// One step's usage; a call without a closing total is the sum of these.
    StepUsage(AgentUsage),
    /// The CLI's closing event: its own total for the call, and the reason
    /// the call failed if it did. A later closing event replaces an earlier
    /// one whole, so the two never come from different events.
    Closing {
        usage: Option<AgentUsage>,
        error: Option<String>,
    },
    /// An error event in the middle of the stream, as a bounded preview for
    /// the log.
    ErrorEvent(String),
}

/// One provider's CLI. See the module documentation.
pub(crate) trait AgentDialect: Sync {
    /// Short name, as in `LLM_PROVIDER` and the job's metadata.
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// The command line for one call.
    fn command(&self, request: &CommandRequest<'_>) -> CommandSpec;

    /// What one complete stdout line means. Most lines mean nothing.
    fn read_line(&self, line: &str) -> Vec<AgentEvent>;

    /// The usage, read from the tail of stdout, when no event gave it. For a
    /// CLI that writes one object at the end rather than a stream.
    fn usage_from_tail(&self, _tail: &str) -> Option<AgentUsage> {
        None
    }
}
