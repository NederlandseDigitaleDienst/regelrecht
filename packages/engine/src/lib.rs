//! RegelRecht Engine
//!
//! A Rust implementation of the RegelRecht law execution engine.
//! This library provides functionality for:
//! - Loading and parsing article-based law specifications (YAML)
//! - Executing law logic with variable resolution
//! - Cross-law references and IoC (open terms) resolution
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{LawExecutionService, Value};
//! use std::collections::HashMap;
//!
//! let mut service = LawExecutionService::new();
//! service.load_law(yaml_str)?;
//!
//! let mut params = HashMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! let result = service.evaluate_law_output(
//!     "wet_op_de_zorgtoeslag",
//!     "heeft_recht_op_zorgtoeslag",
//!     params,
//!     "2024-01-01",
//! )?;
//! ```

pub mod annotation;
pub mod answer;
pub mod article;
pub mod config;
pub mod context;
pub mod data_source;
mod demand;
pub mod engine;
pub mod error;
mod load_check;
pub mod operations;
pub mod priority;
pub mod receipt;
pub mod resolver;
#[cfg(feature = "validate")]
pub mod schema;
pub mod service;
pub mod trace;
pub mod typecheck;
pub mod types;
pub mod units;
pub mod uri;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export commonly used items
pub use annotation::{
    law_id_from_source, resolve as resolve_note, MatchResult, MatchStatus, SelectorHint,
    SkipReason, TextMatch, TextQuoteSelector,
};
pub use answer::{Answer, AnswerProvenance, AnswerRequest, Authority, Call, OpenTermAnswerer};
pub use article::{
    Action, ActionOperation, ActionValue, Article, ArticleBasedLaw, Case, Execution,
    HookDeclaration, HookFilter, HookPoint, LawLoad, MachineReadable, OverrideDeclaration,
    ProcedureDefinition, Source, Stage, UntranslatableEntry,
};
pub use config::{
    MAX_ARRAY_SIZE, MAX_CROSS_LAW_DEPTH, MAX_LOADED_LAWS, MAX_OPERATION_DEPTH, MAX_PROPERTY_DEPTH,
    MAX_YAML_SIZE,
};
pub use context::RuleContext;
pub use data_source::{DataSource, DataSourceMatch, DataSourceRegistry, DictDataSource};
pub use engine::{ArticleEngine, ArticleResult, FilledBy, OpenTermFilled, OutputProvenance};
pub use error::{EngineError, ExternalError, Result};
pub use operations::{evaluate_value, execute_operation, ValueResolver};
pub use receipt::ExecutionReceipt;
pub use resolver::{
    DeclarationKind, DeclarationNotInForce, DeclarationsFromOtherVersion, DelegationRefusal,
    HookEntry, ImplementationLookup, ProcedureMiss, RuleResolver,
};
pub use service::{
    DecisionTerm, ExecutionOutcome, LawExecutionService, LawInfo, ProcedureStage, ServiceProvider,
    StageReport, StageState, Submission, SubmissionArticle,
};
pub use trace::{PathNode, TraceBuilder};
pub use types::{
    Connectivity, LegalStatus, MissingFact, MissingKind, Operation, ParameterType, PathNodeType,
    RegulatoryLayer, ResolveType, UntranslatableMode, Value,
};
pub use uri::{internal_reference, ReferenceType, RegelrechtUri, RegelrechtUriBuilder};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "0.3.0");
    }

    #[test]
    fn test_reexports() {
        // Verify re-exports work
        let _val = Value::Int(42);
        let _op = Operation::Equals;
        let _err = EngineError::DivisionByZero;
    }
}
