//! Editor-API library surface.
//!
//! Only the modules needed by `tests/` are exposed here — the binary
//! (`main.rs`) re-declares the full set, so the lib does not have to
//! pull in modules whose internal items are referenced only from the
//! binary's route registration.

pub mod accounts;
pub mod config;
pub mod corpus_handlers;
pub mod credentials;
pub mod crypto;
pub mod enrich_review;
pub mod feature_flags;
pub mod github_oauth;
mod law_search;
pub mod state;
pub mod task_requests;
pub mod traject_corpus;
pub mod traject_index_diagnosis;
pub mod traject_integrity;
pub mod trajects;
pub mod user_notes;
