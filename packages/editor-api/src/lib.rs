//! The editor-api: the backend of the editor in `frontend/`.
//!
//! [`router`] puts the HTTP routes together with their role gates; the binary
//! (`main.rs`) builds the state, wraps the router in a session layer and serves
//! it. Everything a test needs is reachable from here.

pub mod accounts;
pub mod config;
pub mod corpus_handlers;
pub mod credentials;
pub mod crypto;
pub mod enrich_review;
pub mod favorites;
pub mod feature_flags;
pub mod github_oauth;
pub mod harvest_proxy;
mod law_search;
pub mod middleware;
mod router;
pub mod state;
pub mod static_cache;
pub mod task_requests;
pub mod tasks_api;
pub mod traject_corpus;
pub mod traject_index_diagnosis;
pub mod traject_integrity;
pub mod traject_runs;
pub mod trajects;
pub mod user_notes;
pub mod user_settings;

pub use router::router;
