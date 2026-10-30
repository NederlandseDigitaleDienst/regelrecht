//! # regelrecht-law-model
//!
//! The canonical Rust representation of the law-YAML document model — the shape
//! of the files under `corpus/regulation/`. This crate is a dependency-light
//! leaf (serde + serde_json, no engine/runtime deps) that every consumer of the
//! law format can depend on, so the model is defined **once** instead of being
//! re-derived per crate.
//!
//! Scope: document structs/enums plus allocation-free accessors, and
//! [`stored`]: the lossless JSON form of a `Value` and the rule two stored
//! values are compared by. It deliberately contains no YAML loading, security
//! limits or evaluation logic — the engine owns those
//! (`regelrecht_engine::article::LawLoad` for loading, the resolver for
//! execution) and re-exports these types so existing paths keep working.
//!
//! The JSON schema under `schema/` remains the authoritative specification
//! (RFC-013); this crate is one conforming representation of it.

mod header;
mod model;
pub mod stored;
mod value;

pub use header::{parse_law_header, LawHeader};
pub use model::{
    foreach_binding, is_article_reference, is_fact_reference, is_kind_reference,
    is_variable_reference, Action, ActionOperation, ActionValue, Article, ArticleBasedLaw,
    ArticleReference, ArticleRequirement, AuthorityType, Case, CombineOp, CompetentAuthority,
    Declaration, DeclaredProperty, Definition, Execution, HookDeclaration, HookFilter, HookPoint,
    ImplementsDeclaration, Input, KindDefinition, KindLevel, KindMembership, LegalBasis,
    MachineReadable, Marking, MarkingResolution, Moment, NamedAuthority, OpenTerm, OpenTermDefault,
    Output, OverrideDeclaration, Parameter, Placement, PlacementContainer, Preamble,
    ProcedureAppliesTo, ProcedureDefinition, Produced, ProducedItem, Produces, ProducesList,
    ProvisionReference, ResolveSpec, Source, Stage, StageRequirement, Submission, Temporal,
    TypeSpec, UntranslatableEntry, DEFAULT_FOREACH_AS, SUBMISSION_KINDS,
};
/// The engine reads a version through the model, not from `regelrecht-shared`
/// itself: it sits on the model alone.
pub use regelrecht_shared::parse_schema_version;
pub use value::{MissingFact, MissingKind, Operation, ParameterType, RegulatoryLayer, Value};
