//! Error types for the RegelRecht engine
//!
//! This module provides two error types:
//!
//! - [`EngineError`]: Internal error type with full details for debugging
//! - [`ExternalError`]: Sanitized error type safe for external exposure
//!
//! # Security Considerations
//!
//! Internal errors may contain sensitive information like file paths,
//! internal state, or system details. Use `ExternalError` when returning
//! errors to external callers (API responses, WASM, etc.) to prevent
//! information disclosure.

use crate::trace::PathNode;
use thiserror::Error;

/// Main error type for engine operations
#[derive(Error, Debug)]
pub enum EngineError {
    /// Failed to load or parse a law file
    #[error("Failed to load law: {0}")]
    LoadError(String),

    /// YAML parsing error
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml_ng::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO error (file operations)
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Variable not found during resolution
    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Type mismatch during operation
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    /// Incompatible units combined under an operation (RFC-023).
    #[error("Unit mismatch in {operation}: cannot combine {left} and {right}")]
    UnitMismatch {
        operation: String,
        left: String,
        right: String,
    },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// A law used an absent value (null) where it needs a number, date or truth
    /// value (RFC-036). A legal text never treats "geen" as an amount or a
    /// verdict without saying so; the law has to test for absence first
    /// (`EQUALS … null`). Unknown (a fact nobody has) is the other kind of
    /// nothing and propagates instead of erroring.
    #[error(
        "{operation}: operand is null (absent); a law does not calculate with or decide on \
         'geen' without saying so — test for absence first (EQUALS … null)"
    )]
    AbsentOperand { operation: String },

    /// A `null` reached a parameter or input the law declares as never absent
    /// (RFC-036). The declaration (`nullable`, default false) is the law's
    /// claim that "geen" is not a value this field can take; a register, a
    /// caller or another law that delivers `null` anyway contradicts that
    /// claim, and the contradiction is reported at the boundary where it
    /// arises rather than three operations later as an `AbsentOperand`.
    /// `origin` names where the null came from: `source <name>`,
    /// `<law>.<output>`, `skipped call to <law>`, `parameter from <law>` or
    /// `lookup keyed on null`.
    #[error(
        "{origin} delivered null for '{field}' of {law_id}, which the law declares as never \
         absent; declare the field nullable if absence is a legitimate value (RFC-036)"
    )]
    NullForNonNullable {
        law_id: String,
        field: String,
        origin: String,
    },

    /// An output the law declares as never absent evaluated to `null`
    /// (RFC-036): an `IF` without a matching case and without `default`, a
    /// `MIN`/`MAX` over an empty collection, a `null` input passed through.
    /// The law either has to say what the value is in that case (a default)
    /// or declare that the output may be absent.
    #[error(
        "output '{output}' of {law_id} evaluated to null (absent) but is not declared nullable; \
         add a default or declare the output nullable (RFC-036)"
    )]
    NullOutput { law_id: String, output: String },

    /// A finding was given that no article in the execution asks: none of
    /// them leaves that open term to an authority per case
    /// (`decided_per_case_by`). `findings` names each such finding, so a
    /// caller can leave them out and say so.
    #[error(
        "no article in this execution leaves {} to an authority per case \
         (decided_per_case_by), so the finding is asked by none",
        .findings.iter().map(|id| format!("'{id}'")).collect::<Vec<_>>().join(", ")
    )]
    FindingsNotAsked { findings: Vec<String> },

    /// Invalid URI format
    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    /// Law not found
    #[error("Law not found: {0}")]
    LawNotFound(String),

    /// Law exists but no version is in force yet on the reference date (RFC-019 §3).
    /// States the data fact only — never a legal verdict.
    #[error("No version of law '{law_id}' in force on {reference_date} (not yet in force)")]
    LawNotYetInForce {
        law_id: String,
        reference_date: String,
    },

    /// The law's most recent version ended before the reference date (RFC-019 §3).
    /// States the data fact only — never a verdict like "geen grondslag" or
    /// "vervallen zonder opvolger": eerbiedigende werking, a statische verwijzing,
    /// or an alternative grondslag may keep the law applicable (RFC-020).
    #[error(
        "No version of law '{law_id}' in force on {reference_date}; last in force until {valid_to}"
    )]
    LawEnded {
        law_id: String,
        reference_date: String,
        valid_to: String,
    },

    /// Article not found
    #[error("Article not found: {law_id}#{article}")]
    ArticleNotFound { law_id: String, article: String },

    /// Output not found in article
    #[error("Output '{output}' not found in law '{law_id}'")]
    OutputNotFound { law_id: String, output: String },

    /// An output the law says does not arise.
    ///
    /// Distinct from [`EngineError::OutputNotFound`], which is the shape of a
    /// misspelled name or a broken binding. "Bestaat geen aanspraak" is an
    /// answer the law gives, and one a consuming law has to be able to act on:
    /// reporting it as a missing output tells an operator to go looking for a
    /// modelling error that is not there, and leaves the citizen without the
    /// ground for a decision that does exist.
    #[error("Output '{output}' of law '{law_id}' does not arise: {grounds} ({voided_by} article {article})")]
    OutputVoided {
        law_id: String,
        output: String,
        voided_by: String,
        article: String,
        grounds: String,
    },

    /// Circular reference detected
    #[error("Circular reference detected: {0}")]
    CircularReference(String),

    /// A parameter of the law being evaluated was passed as `null` although
    /// the law does not declare it nullable, or a required parameter was
    /// passed as an unknown (RFC-036). At the top level that is the caller's
    /// error: the lookup key names nobody, so no register can be asked and
    /// no unknown fact can be named, and a parameter that is not nullable
    /// never takes `null`. Across laws a `null` required parameter is not an
    /// error but a skip (the target is not run; the input is that `null` or
    /// unknown), so this is raised at depth 0 only.
    #[error(
        "parameter '{name}' of {law_id} is {value}: a law cannot be evaluated for nobody, and a \
         parameter that is not declared nullable never takes null"
    )]
    MissingParameter {
        law_id: String,
        name: String,
        /// `null` or `unknown`: how the caller left the parameter empty.
        value: String,
    },

    /// Arithmetic overflow when converting f64 to i64
    #[error("Arithmetic overflow: {0}")]
    ArithmeticOverflow(String),

    /// Maximum operation nesting depth exceeded
    #[error("Maximum operation depth exceeded: {0} levels")]
    MaxDepthExceeded(usize),

    /// Resolution error (IoC open term resolution, priority conflicts, etc.)
    #[error("Resolution error: {0}")]
    ResolutionError(String),

    /// Data source error
    #[error("Data source error: {0}")]
    DataSourceError(String),

    /// Invalid date format
    #[error("Invalid date format: {0}")]
    InvalidDate(String),

    /// Article contains untranslatable constructs (RFC-012)
    #[error("Untranslatable construct in {law_id} article {article}: {construct} — {reason}")]
    Untranslatable {
        law_id: String,
        article: String,
        construct: String,
        reason: String,
    },

    /// External reference not resolved - requires ServiceProvider
    #[error(
        "External reference not resolved: input '{input_name}' requires resolution from \
         regulation '{regulation}' output '{output}'. \
         Pass the value as a parameter or use ServiceProvider."
    )]
    ExternalReferenceNotResolved {
        input_name: String,
        regulation: String,
        output: String,
    },

    /// Wraps an error that occurred during traced execution, carrying the partial trace.
    ///
    /// This variant is returned by `evaluate_law_output_with_trace` when execution
    /// fails, so callers can inspect the trace up to the point of failure.
    #[error("{source}")]
    TracedError {
        source: Box<EngineError>,
        trace: Option<Box<PathNode>>,
    },
}

/// Result type alias for engine operations
pub type Result<T> = std::result::Result<T, EngineError>;

/// External-safe error type that sanitizes internal details.
///
/// Use this type when returning errors to external callers (API responses,
/// WASM bindings, etc.) to prevent information disclosure.
///
/// # Example
///
/// ```
/// use regelrecht_engine::error::{EngineError, ExternalError};
///
/// fn handle_request() -> Result<(), ExternalError> {
///     let internal_result: Result<(), EngineError> = Err(EngineError::IoError(
///         std::io::Error::new(std::io::ErrorKind::NotFound, "secret/path/file.yaml")
///     ));
///
///     // Convert to external error (sanitizes path)
///     internal_result.map_err(ExternalError::from)
/// }
/// ```
#[derive(Error, Debug)]
pub enum ExternalError {
    /// Failed to load law configuration
    #[error("Failed to load law configuration")]
    LoadError,

    /// YAML parsing failed
    #[error("Invalid law format")]
    ParseError,

    /// Variable not found during resolution
    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Type mismatch during operation
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    /// Incompatible units combined under an operation (RFC-023).
    #[error("Unit mismatch in {operation}: cannot combine {left} and {right}")]
    UnitMismatch {
        operation: String,
        left: String,
        right: String,
    },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// A law used an absent value (null) where it needs a number, date or truth
    /// value (RFC-036). The operation name is safe to expose: it names a construct
    /// in the law, not internal state.
    #[error(
        "{operation}: operand is null (absent); a law does not calculate with or decide on \
         'geen' without saying so — test for absence first (EQUALS … null)"
    )]
    AbsentOperand { operation: String },

    /// A `null` reached a field the law declares as never absent (RFC-036).
    /// Law id, field and origin name constructs in the law, not internal state.
    #[error(
        "{origin} delivered null for '{field}' of {law_id}, which the law declares as never \
         absent; declare the field nullable if absence is a legitimate value (RFC-036)"
    )]
    NullForNonNullable {
        law_id: String,
        field: String,
        origin: String,
    },

    /// An output the law declares as never absent evaluated to `null` (RFC-036).
    #[error(
        "output '{output}' of {law_id} evaluated to null (absent) but is not declared nullable; \
         add a default or declare the output nullable (RFC-036)"
    )]
    NullOutput { law_id: String, output: String },

    /// Invalid URI format
    #[error("Invalid URI format")]
    InvalidUri,

    /// Law not found
    #[error("Law not found: {0}")]
    LawNotFound(String),

    /// Law exists but no version is in force yet on the reference date (RFC-019 §3).
    /// Carries the same public data facts as the internal error: honest
    /// diagnostics apply at the external boundary too.
    #[error("No version of law '{law_id}' in force on {reference_date} (not yet in force)")]
    LawNotYetInForce {
        law_id: String,
        reference_date: String,
    },

    /// The law's most recent version ended before the reference date (RFC-019 §3).
    #[error(
        "No version of law '{law_id}' in force on {reference_date}; last in force until {valid_to}"
    )]
    LawEnded {
        law_id: String,
        reference_date: String,
        valid_to: String,
    },

    /// Article not found
    #[error("Article not found in law")]
    ArticleNotFound,

    /// Output not found
    #[error("Output not found: {0}")]
    OutputNotFound(String),

    /// The law says this output does not arise, with the words that say so.
    #[error("Output '{output}' does not arise: {grounds}")]
    OutputVoided { output: String, grounds: String },

    /// Circular reference detected
    #[error("Circular reference detected")]
    CircularReference,

    /// Required parameter missing
    #[error("Required parameter missing: {0}")]
    MissingParameter(String),

    /// Arithmetic overflow
    #[error("Arithmetic overflow")]
    ArithmeticOverflow,

    /// Maximum depth exceeded
    #[error("Maximum nesting depth exceeded")]
    MaxDepthExceeded,

    /// Resolution error (IoC open term resolution failed)
    #[error("Resolution failed")]
    ResolutionError,

    /// External reference not resolved
    #[error("External reference not resolved: {0}")]
    ExternalReferenceNotResolved(String),

    /// Data source error
    #[error("Data source error")]
    DataSourceError,

    /// Invalid date format
    #[error("Invalid date format")]
    InvalidDate,

    /// Article contains untranslatable constructs (RFC-012)
    #[error("Untranslatable construct: {0}")]
    Untranslatable(String),
}

impl From<EngineError> for ExternalError {
    fn from(err: EngineError) -> Self {
        // Log the internal error for debugging (if tracing is configured)
        tracing::debug!(internal_error = ?err, "Converting internal error to external");

        match err {
            EngineError::LoadError(_) | EngineError::IoError(_) => ExternalError::LoadError,
            EngineError::YamlError(_) | EngineError::JsonError(_) => ExternalError::ParseError,
            EngineError::VariableNotFound(name) => ExternalError::VariableNotFound(name),
            EngineError::InvalidOperation(msg) => ExternalError::InvalidOperation(msg),
            // Outside the engine it stays the operation it was before it got
            // a variant of its own: the message names the findings.
            e @ EngineError::FindingsNotAsked { .. } => {
                ExternalError::InvalidOperation(e.to_string())
            }
            EngineError::TypeMismatch { expected, actual } => {
                ExternalError::TypeMismatch { expected, actual }
            }
            EngineError::UnitMismatch {
                operation,
                left,
                right,
            } => ExternalError::UnitMismatch {
                operation,
                left,
                right,
            },
            EngineError::DivisionByZero => ExternalError::DivisionByZero,
            EngineError::AbsentOperand { operation } => ExternalError::AbsentOperand { operation },
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => ExternalError::NullForNonNullable {
                law_id,
                field,
                origin,
            },
            EngineError::NullOutput { law_id, output } => {
                ExternalError::NullOutput { law_id, output }
            }
            EngineError::InvalidUri(_) => ExternalError::InvalidUri,
            EngineError::LawNotFound(id) => ExternalError::LawNotFound(id),
            // RFC-019 §3: the validity facts are public legal data, not internal
            // detail - pass them through to external consumers (WASM/API) too.
            EngineError::LawNotYetInForce {
                law_id,
                reference_date,
            } => ExternalError::LawNotYetInForce {
                law_id,
                reference_date,
            },
            EngineError::LawEnded {
                law_id,
                reference_date,
                valid_to,
            } => ExternalError::LawEnded {
                law_id,
                reference_date,
                valid_to,
            },
            EngineError::ArticleNotFound { .. } => ExternalError::ArticleNotFound,
            EngineError::OutputNotFound { output, .. } => ExternalError::OutputNotFound(output),
            // The ground travels with it: an API caller has to be able to tell
            // "the law says no" from "this output does not exist".
            EngineError::OutputVoided {
                output, grounds, ..
            } => ExternalError::OutputVoided { output, grounds },
            EngineError::CircularReference(_) => ExternalError::CircularReference,
            EngineError::MissingParameter { name, .. } => ExternalError::MissingParameter(name),
            EngineError::ArithmeticOverflow(_) => ExternalError::ArithmeticOverflow,
            EngineError::MaxDepthExceeded(_) => ExternalError::MaxDepthExceeded,
            EngineError::ResolutionError(_) => ExternalError::ResolutionError,
            EngineError::ExternalReferenceNotResolved { input_name, .. } => {
                ExternalError::ExternalReferenceNotResolved(input_name)
            }
            EngineError::DataSourceError(_) => ExternalError::DataSourceError,
            EngineError::InvalidDate(_) => ExternalError::InvalidDate,
            EngineError::Untranslatable { construct, .. } => {
                ExternalError::Untranslatable(construct)
            }
            EngineError::TracedError { source, .. } => ExternalError::from(*source),
        }
    }
}

// WASM error conversion
#[cfg(feature = "wasm")]
impl From<EngineError> for wasm_bindgen::JsValue {
    fn from(err: EngineError) -> Self {
        wasm_bindgen::JsValue::from_str(&err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EngineError::VariableNotFound("test_var".to_string());
        assert_eq!(err.to_string(), "Variable not found: test_var");
    }

    #[test]
    fn test_type_mismatch_display() {
        let err = EngineError::TypeMismatch {
            expected: "number".to_string(),
            actual: "string".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Type mismatch: expected number, got string"
        );
    }

    #[test]
    fn test_absent_operand_names_the_operation_and_the_remedy() {
        let err = EngineError::AbsentOperand {
            operation: "GREATER_THAN".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.starts_with("GREATER_THAN: operand is null (absent)"),
            "{msg}"
        );
        assert!(msg.contains("EQUALS … null"), "{msg}");
        // The operation name is part of the law, so the external error keeps it.
        let external: ExternalError = err.into();
        assert_eq!(external.to_string(), msg);
    }

    #[test]
    fn test_null_for_non_nullable_names_origin_field_law_and_remedy() {
        let err = EngineError::NullForNonNullable {
            law_id: "wet_op_de_huurtoeslag".to_string(),
            field: "huur".to_string(),
            origin: "source huurregister".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.starts_with(
                "source huurregister delivered null for 'huur' of wet_op_de_huurtoeslag"
            ),
            "{msg}"
        );
        assert!(msg.contains("declares as never absent"), "{msg}");
        assert!(msg.contains("declare the field nullable"), "{msg}");
        let external: ExternalError = err.into();
        assert_eq!(external.to_string(), msg);
    }

    #[test]
    fn test_null_output_names_output_law_and_remedy() {
        let err = EngineError::NullOutput {
            law_id: "wet_op_de_huurtoeslag".to_string(),
            output: "huurklasse".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.starts_with("output 'huurklasse' of wet_op_de_huurtoeslag evaluated to null"),
            "{msg}"
        );
        assert!(msg.contains("not declared nullable"), "{msg}");
        assert!(msg.contains("add a default"), "{msg}");
        let external: ExternalError = err.into();
        assert_eq!(external.to_string(), msg);
    }

    #[test]
    fn test_external_error_sanitizes_paths() {
        // IoError with path should be sanitized
        let internal = EngineError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "/secret/internal/path/file.yaml",
        ));
        let external: ExternalError = internal.into();

        let msg = external.to_string();
        assert!(!msg.contains("/secret"), "Path should be sanitized");
        assert!(!msg.contains("file.yaml"), "Filename should be sanitized");
        assert_eq!(msg, "Failed to load law configuration");
    }

    #[test]
    fn test_external_error_preserves_safe_info() {
        // Variable names are safe to expose
        let internal = EngineError::VariableNotFound("user_age".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Variable not found: user_age");

        // Law IDs are safe to expose
        let internal = EngineError::LawNotFound("wet_op_de_zorgtoeslag".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Law not found: wet_op_de_zorgtoeslag");
    }

    #[test]
    fn test_external_error_hides_internal_details() {
        // CircularReference details are hidden
        let internal =
            EngineError::CircularReference("Complex circular chain: a -> b -> c -> a".to_string());
        let external: ExternalError = internal.into();
        assert_eq!(external.to_string(), "Circular reference detected");

        // ArticleNotFound hides law/article details
        let internal = EngineError::ArticleNotFound {
            law_id: "internal_law".to_string(),
            article: "secret_article".to_string(),
        };
        let external: ExternalError = internal.into();
        assert!(!external.to_string().contains("internal_law"));
        assert!(!external.to_string().contains("secret_article"));
    }
}
