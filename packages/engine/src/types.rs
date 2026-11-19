//! Engine runtime types.
//!
//! `Value`, `Operation`, `ParameterType` and `RegulatoryLayer` are the
//! document-model types: they now live in the dependency-light
//! [`regelrecht_law_model`] crate and are re-exported here so the historical
//! `crate::types::*` paths keep working unchanged. The enums defined below
//! describe engine runtime / execution-trace concerns, not the document format,
//! so they stay in the engine.

use serde::{Deserialize, Serialize};

/// Re-export the canonical document-model value types from the law-model crate.
pub use regelrecht_law_model::{
    MissingFact, MissingKind, Number, Operation, ParameterType, RegulatoryLayer, Rounding,
    TypeSpec, Value,
};

/// A number from its decimal notation, for tests.
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) fn dec(text: &str) -> Number {
    text.parse().expect("test literal should be a number")
}

/// How the engine handles an article that flags a construct (RFC-012).
///
/// Controls runtime behavior when an article declares a construct that cannot
/// be faithfully expressed. Both channels count: `untranslatables` on schema
/// v0.5.x and `markings` from v0.7.0 onwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UntranslatableMode {
    /// Hard error on any unaccepted flagged construct. Accepted ones execute partial logic.
    #[default]
    Error,
    /// Execute partial logic. Outputs from articles that flag a construct carry an
    /// `UNTRANSLATABLE` taint that propagates through downstream operations (like NaN).
    Propagate,
    /// Execute partial logic, log warning in trace. No taint propagation.
    Warn,
    /// Execute partial logic silently. Only valid for entries with `accepted: true` —
    /// unaccepted ones still error.
    Ignore,
}

impl std::str::FromStr for UntranslatableMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "error" => Ok(UntranslatableMode::Error),
            "propagate" => Ok(UntranslatableMode::Propagate),
            "warn" => Ok(UntranslatableMode::Warn),
            "ignore" => Ok(UntranslatableMode::Ignore),
            _ => Err(format!(
                "unknown untranslatable mode '{s}', expected: error, propagate, warn, ignore"
            )),
        }
    }
}

/// Engine connectivity mode — whether this engine resolves cross-law references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Connectivity {
    /// Engine runs standalone, no cross-law resolution.
    Solo,
}

/// Legal status of execution results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LegalStatus {
    /// Results are for simulation/testing purposes only.
    Simulation,
}

/// Node type in execution trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathNodeType {
    /// Variable/value resolution step
    Resolve,
    /// Operation execution (e.g., ADD, EQUALS)
    Operation,
    /// Action execution within an article
    Action,
    /// Requirement check
    Requirement,
    /// Cross-law reference resolution (source.regulation lookup)
    CrossLawReference,
    /// Article-level execution
    Article,
    /// Cached cross-law result (memoized)
    Cached,
    /// Open term resolution via IoC (implements lookup)
    OpenTermResolution,
    /// Hook resolution (lifecycle hook firing, RFC-007)
    HookResolution,
    /// Override resolution (lex specialis replacement, RFC-007)
    OverrideResolution,
}

/// Resolve type for variable resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolveType {
    /// Value resolved from a regelrecht:// URI
    Uri,
    /// Value resolved from input parameters
    Parameter,
    /// Value resolved from article definitions (constants)
    Definition,
    /// Value resolved from calculated outputs
    Output,
    /// Value resolved from input specification
    Input,
    /// Value resolved from local scope (loop variables)
    Local,
    /// Value resolved from context variables (referencedate)
    Context,
    /// Value resolved from cached cross-law results
    ResolvedInput,
    /// Value resolved from external data source
    DataSource,
    /// Value resolved via open term implementation (IoC)
    OpenTerm,
    /// The delegating law's default for an open term, taken because the
    /// implementing regulation returned null for this case (RFC-036: the
    /// implementation is silent, no deviation was granted)
    OpenTermSilent,
    /// Value resolved via lifecycle hook (RFC-007)
    Hook,
    /// Value resolved via lex specialis override (RFC-007)
    Override,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mode string arrives from the `--untranslatable=` flag of the
    /// evaluate binary and from BDD steps. A mode that parses to the wrong
    /// variant changes how flagged articles execute, so every spelling maps to
    /// its own variant and none falls back to the default.
    #[test]
    fn untranslatable_mode_parses_each_mode_to_its_own_variant() {
        let cases = [
            ("error", UntranslatableMode::Error),
            ("propagate", UntranslatableMode::Propagate),
            ("warn", UntranslatableMode::Warn),
            ("ignore", UntranslatableMode::Ignore),
        ];
        for (input, expected) in cases {
            assert_eq!(input.parse::<UntranslatableMode>(), Ok(expected), "{input}");
        }
    }

    /// An unknown mode is an error, not the default: a typo in the flag must
    /// not silently run in `Error` mode while the caller believes it chose
    /// another.
    #[test]
    fn untranslatable_mode_rejects_unknown_mode() {
        let err = "propogate".parse::<UntranslatableMode>().unwrap_err();
        assert!(err.contains("propogate"), "{err}");
    }
}
