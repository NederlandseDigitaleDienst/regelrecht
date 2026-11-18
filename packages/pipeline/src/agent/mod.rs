//! Running an agent CLI: one dialect per provider, and the supervisor that
//! runs any of them.
//!
//! A provider's facts (how to start it, what it can do, what its output means)
//! live in its own file, [`claude`] or [`opencode`], behind the
//! [`AgentDialect`](dialect::AgentDialect) trait. The supervisor starts the
//! process and watches it the same way for every provider. Adding a provider
//! is a new dialect and a variant of [`LlmProvider`]; nothing in the
//! supervisor changes.
//!
//! The fixtures in `tests/fixtures/agent-streams/` play recorded output of
//! each provider through the supervisor (`agent_stream_fixtures.rs`).

pub(crate) mod claude;
pub(crate) mod dialect;
pub(crate) mod opencode;
mod supervisor;

use std::path::Path;

pub use dialect::ToolPolicy;
pub(crate) use dialect::{AgentDialect, AgentEvent, StepFinish};
pub use supervisor::AgentRun;
pub(crate) use supervisor::run_llm_subprocess;

use crate::enrich::LlmProvider;

impl LlmProvider {
    /// The dialect of this provider's CLI. The one place that dispatches on
    /// the provider's behaviour; everything else asks the dialect.
    pub(crate) fn dialect(&self) -> &'static dyn AgentDialect {
        match self {
            LlmProvider::OpenCode { .. } => &opencode::OpenCodeDialect,
            LlmProvider::Claude { .. } => &claude::ClaudeDialect,
        }
    }

    /// The binary to run.
    pub(crate) fn path(&self) -> &Path {
        match self {
            LlmProvider::OpenCode { path, .. } | LlmProvider::Claude { path, .. } => path,
        }
    }

    /// The configured model, if any.
    pub(crate) fn model(&self) -> Option<&str> {
        match self {
            LlmProvider::OpenCode { model, .. } | LlmProvider::Claude { model, .. } => {
                model.as_deref()
            }
        }
    }
}
