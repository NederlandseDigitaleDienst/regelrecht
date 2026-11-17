//! Shared telemetry helpers for regelrecht binaries.
//!
//! Each binary previously hand-rolled the same `tracing_subscriber::fmt()`
//! initialization. [`init_subscriber`] consolidates that pattern.

use std::env::VarError;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

/// Install the default `tracing_subscriber::fmt` global subscriber, honoring
/// `RUST_LOG` and falling back to `default_level` when it is unset.
///
/// Uses `try_init()` so a second call (e.g. from an integration test that
/// already set up a subscriber) leaves the existing subscriber intact rather
/// than panicking. The error is reported on stderr because the very subscriber
/// we tried to install is unavailable at that moment.
///
/// Span events are off (`FmtSpan::NONE`) — the pre-existing behaviour every
/// caller had. See [`init_subscriber_with_spans`] to turn them on.
pub fn init_subscriber(default_level: &str) {
    init_subscriber_with_spans(default_level, false);
}

/// Like [`init_subscriber`], but `default_span_events` chooses the fallback
/// when `LOG_SPAN_EVENTS` is unset.
///
/// ## Span-close timing logs
///
/// With span events on (`FmtSpan::CLOSE`), a log line is emitted when each
/// `tracing` span closes, carrying its `time.busy`/`time.idle` duration.
/// This turns the `#[tracing::instrument]` spans on the editor's
/// write/build path into a per-step latency breakdown in `zad logs`.
///
/// It is **opt-in per service** on purpose: `init_subscriber` is shared by
/// hot-path services (the harvest/enrich workers, the pipeline API), where
/// a close event per span would multiply log volume for no benefit — they
/// keep the `default_span_events = false` default. Only services that want
/// the breakdown (editor-api) call this with `true`.
///
/// `LOG_SPAN_EVENTS`, when set, always wins over `default_span_events` — so
/// an operator can force spans on (`close`/`new`/`active`/`full`) or off
/// (`none`, or any unrecognised value) without a redeploy. Passing the
/// default as an argument rather than mutating the process environment
/// avoids the `set_var`-under-a-running-runtime data-race caveat and keeps
/// the setting from leaking into child processes (git subprocesses, etc.).
///
/// ## Output format
///
/// `LOG_FORMAT` selects the formatter: `json` emits one JSON object per event
/// (per-field searchable in a log backend), anything else — including unset —
/// keeps the human-readable text lines that are pleasant during local
/// development. Deployments that ship logs to a collector set `LOG_FORMAT=json`.
pub fn init_subscriber_with_spans(default_level: &str, default_span_events: bool) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    let span_events = resolve_span_events(default_span_events);

    let result = match resolve_log_format() {
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_span_events(span_events)
            .json()
            // Lift the event's own fields (incl. `message`) to the top level
            // instead of nesting them under `fields`, and keep the enclosing
            // span context so a JSON line carries the same information the
            // text line showed inline.
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true)
            .try_init(),
        LogFormat::Text => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_span_events(span_events)
            .try_init(),
    };

    if let Err(e) = result {
        eprintln!("warning: tracing subscriber already initialized: {e}");
    }
}

/// Log output format, selected by `LOG_FORMAT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFormat {
    /// Human-readable text lines — the default, and what local development wants.
    Text,
    /// One JSON object per event, for per-field searching in a log backend.
    Json,
}

/// Read `LOG_FORMAT` from the environment and map it onto a [`LogFormat`].
fn resolve_log_format() -> LogFormat {
    match std::env::var("LOG_FORMAT") {
        Ok(raw) => parse_log_format(Some(&raw)),
        // Set but not valid UTF-8: it cannot name a format, so fall back to text.
        Err(VarError::NotUnicode(_)) => {
            eprintln!("LOG_FORMAT is not valid UTF-8; falling back to text");
            LogFormat::Text
        }
        Err(VarError::NotPresent) => parse_log_format(None),
    }
}

/// Pure mapping of a `LOG_FORMAT` value onto a [`LogFormat`], so the choice is
/// testable without touching the process environment.
///
/// Only `json` switches formatter. `text`/`plain` name the default explicitly;
/// any other value falls back to text — a typo must never silence logs.
fn parse_log_format(raw: Option<&str>) -> LogFormat {
    match raw.map(str::trim) {
        None | Some("") => LogFormat::Text,
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "json" => LogFormat::Json,
            "text" | "plain" => LogFormat::Text,
            other => {
                eprintln!("warning: unrecognised LOG_FORMAT={other:?}, falling back to text");
                LogFormat::Text
            }
        },
    }
}

/// Resolve the [`FmtSpan`] mask: a *set* `LOG_SPAN_EVENTS` always wins;
/// only a genuinely absent variable falls back to the caller's `default_on`
/// (CLOSE when on, NONE when off).
fn resolve_span_events(default_on: bool) -> FmtSpan {
    match std::env::var("LOG_SPAN_EVENTS") {
        Ok(raw) => match raw.to_ascii_lowercase().as_str() {
            "close" => FmtSpan::CLOSE,
            "new" => FmtSpan::NEW,
            "active" => FmtSpan::ACTIVE,
            "full" => FmtSpan::FULL,
            // Explicit off (or any unrecognised value) — never span events.
            _ => FmtSpan::NONE,
        },
        // The variable is set but not valid UTF-8: it is still an explicit
        // (if garbled) override, so treat it as "unrecognised" → off, never
        // silently fall back to `default_on`.
        Err(VarError::NotUnicode(_)) => FmtSpan::NONE,
        // Genuinely unset: honour the per-service default.
        Err(VarError::NotPresent) if default_on => FmtSpan::CLOSE,
        Err(VarError::NotPresent) => FmtSpan::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_log_format_is_text() {
        assert_eq!(parse_log_format(None), LogFormat::Text);
    }

    #[test]
    fn empty_log_format_is_text() {
        assert_eq!(parse_log_format(Some("")), LogFormat::Text);
        assert_eq!(parse_log_format(Some("   ")), LogFormat::Text);
    }

    #[test]
    fn json_selects_json_case_insensitively() {
        assert_eq!(parse_log_format(Some("json")), LogFormat::Json);
        assert_eq!(parse_log_format(Some("JSON")), LogFormat::Json);
        assert_eq!(parse_log_format(Some(" Json ")), LogFormat::Json);
    }

    #[test]
    fn text_and_plain_select_text() {
        assert_eq!(parse_log_format(Some("text")), LogFormat::Text);
        assert_eq!(parse_log_format(Some("plain")), LogFormat::Text);
    }

    #[test]
    fn unrecognised_value_never_silences_logs() {
        assert_eq!(parse_log_format(Some("jsonl")), LogFormat::Text);
        assert_eq!(parse_log_format(Some("logfmt")), LogFormat::Text);
    }
}
