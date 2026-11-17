//! `regelrecht-github` — one GitHub REST client shared across all regelrecht
//! applications.
//!
//! # Why hand-rolled (no octocrab)
//!
//! The surface regelrecht touches is small and fixed: a handful of Trees /
//! Contents / Refs / Compare / Pulls / archive calls. A full client like
//! `octocrab` would pull in a large dependency tree (and its own reqwest
//! version) to wrap endpoints we never call. This crate keeps the footprint to
//! the exact calls in use, with error and token types under our own control —
//! reconsider if we start needing PR reviews, GraphQL, or webhooks.
//!
//! # reqwest 0.12 / 0.13 split
//!
//! This crate uses **reqwest 0.13**, matching `corpus`/`harvester`/`pipeline`.
//! `admin`/`auth`/`editor-api` are pinned to **reqwest 0.12** by
//! `openidconnect`/`oauth2`. The two coexist in the workspace on purpose (see
//! the note in `packages/Cargo.toml`); aligning them means upgrading
//! openidconnect, which is out of scope. The editor-api therefore links both:
//! its own 0.12 (OAuth flow) and this crate's 0.13 (REST calls).
//!
//! ## Known remaining REST duplication
//!
//! The editor-api's GitHub OAuth flow (`editor-api/src/github_oauth.rs`) still
//! speaks reqwest 0.12 directly for the OAuth dance and the `/user` lookup. It
//! can only migrate onto this client after the openidconnect upgrade, so it
//! stays a deliberate, documented duplication rather than a forced 0.12↔0.13
//! bridge.
//!
//! # The client
//!
//! One [`GithubClient`] holds a shared `reqwest::Client`, one header builder,
//! one API base (given to [`GithubClient::from_base`], or read from
//! `GITHUB_API_BASE` by [`GithubClient::new`]), and ETag + rate-limit state behind a
//! `std::sync::Mutex` so every method takes `&self`. The lock is never held
//! across a `.await`.

mod activity;
mod archive;
mod client;
mod compare;
mod contents;
pub mod error;
mod pulls;
mod refs;
mod repo_access;
mod trees;

pub use client::GithubClient;
pub use contents::{Committer, DirectoryEntry};
pub use error::{GithubError, Result};
pub use pulls::PrInfo;
pub use repo_access::{RepoAccessError, RepoInfo};
pub use trees::TreeEntryFile;
