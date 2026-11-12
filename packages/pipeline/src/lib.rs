//! PostgreSQL-backed job queue, law status tracking, and the pipeline workers
//! (harvest, enrich, document/law-convert).
//!
//! # Het worker/traject-contract
//!
//! **Pipeline-workers schrijven nooit met een server-token naar een
//! traject-repo.** Een traject-write loopt altijd via een review-taak: de
//! worker levert zijn resultaat op als job-blob + taak
//! (`deliver: "task"`, zie o.a. [`worker::finish_enrich_task_job`] en
//! [`worker::finish_document_convert_task_job`]), de gebruiker keurt goed in
//! de editor, en de editor volgt daarbij de token-voorrangsregel bij het
//! committen (service-token indien geconfigureerd, anders het eigen
//! OAuth-token van de gebruiker; client-gedreven via de save-endpoints van
//! editor-api).
//!
//! Het corpus-token (`CORPUS_GIT_TOKEN`, via `CorpusConfig`) is uitsluitend
//! voor de **centrale** corpus-repo — de operator-repo waar de klassieke
//! corpus-brede enrich zijn `enrich/*`-branches pusht. Guards borgen dit op
//! jobniveau, niet op call-site-conventie:
//!
//! - `document_convert`: [`document_convert::DocumentConvertPayload::require_task_delivery`]
//!   — een job zonder `deliver: "task"` + `requested_by` faalt terminaal vóór
//!   de conversie; er bestaat geen push-pad meer in die module.
//! - `law_convert`: kent alléén de taak-flow (zelfde gate in `worker.rs`).
//! - `enrich`: de taak-flow buigt af naar blob + taak; het corpus-brede pad
//!   weigert traject-gerichte payloads via
//!   [`enrich::EnrichPayload::require_corpus_wide_target`].
//! - `traject_harvest`: schrijft in een werkdirectory en ketent een
//!   taak-flow-enrich — raakt zelf geen traject-repo aan.

pub(crate) mod agent;
pub mod agent_activity;
// Two attributes rather than `all(test, unix)`, as in `enrich.rs`: cargo-mutants
// skips a module marked with a plain `#[cfg(test)]`.
#[cfg(test)]
#[cfg(unix)]
mod agent_stream_fixtures;
pub mod api;
mod api_state;
pub mod config;
pub mod db;
pub mod document_convert;
pub mod enrich;
pub mod enrich_v2;
pub mod error;
pub mod feature_flags;
pub mod harvest;
pub mod harvest_request;
pub mod health;
pub mod job_queue;
pub mod law_convert;
pub mod law_migrate;
pub mod law_status;
pub mod markings;
pub mod models;
pub mod tasks;
#[cfg(feature = "test-utils")]
pub mod test_utils;
pub mod traject_harvest;
pub mod traject_runs;
pub mod untranslatables;
pub mod worker;

pub use api_state::ApiState;

pub use config::{PipelineConfig, WorkerConfig};
pub use db::{MIGRATION_LOCK_KEY, create_pool, ensure_schema};
pub use enrich::{
    ENRICH_PROVIDERS, EnrichConfig, EnrichPayload, EnrichResult, EnrichmentMetadata,
    EnrichmentResultEnvelope, LlmProvider, LlmRunner, ProcessLlmRunner, RelatedLegislation,
    progress_file_path,
};
pub use error::PipelineError;
pub use harvest::{HarvestPayload, HarvestResult, MAX_HARVEST_DEPTH};
// Deliberately no crate-root re-export of the `request_harvest` fn: the axum
// handler `api::harvest::request_harvest` shares that name, so callers import
// it path-qualified via the module.
pub use harvest_request::{HarvestRequestOptions, HarvestRequestOutcome};
pub use models::{
    FeatureFlag, Job, JobStatus, JobType, LawEntry, LawStatusValue, Priority, Untranslatable,
};
