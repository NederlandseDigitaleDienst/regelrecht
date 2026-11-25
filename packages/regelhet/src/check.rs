//! `regelrecht-regelhet check <manifest> [case]`: whether a case starts,
//! without starting a server.
//!
//! The check takes the path the server takes at start ([`app::build_with`]),
//! in test mode with the store in memory and the test login, so it fails on
//! what a start in test mode fails on. What only stable mode refuses, such
//! as a login the case needs, it does not see. It reads no environment: a
//! `REGELHET_STORE` or `OIDC_*` on the machine does not change the verdict.

use std::path::Path;

use chrono::NaiveDate;

use crate::app::{self, StartupError};
use crate::case;
use crate::config::{Config, ConfigError};
use crate::login::Login;

/// What a case that starts is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// The id of the case.
    pub id: String,
    /// Its title, as a citizen reads it.
    pub title: String,
    /// The cell that executes it, by id and by name.
    pub executor: String,
    pub executor_name: String,
    /// The article an application is made under, and the outcome decided.
    pub article: String,
    pub outcome: String,
    /// Every cell of the case, the executor first.
    pub cells: Vec<String>,
    /// The day test mode starts on.
    pub start_date: NaiveDate,
    /// What loading said without refusing the case, such as a regelwerk
    /// version set aside.
    pub messages: Vec<String>,
}

/// Why a case does not start.
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("the case: {0}")]
    Case(#[from] case::CaseError),
    #[error("the configuration: {0}")]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Startup(#[from] StartupError),
}

/// Whether the case `chosen` (the name of its case file) of the manifest at
/// `manifest` starts, or without a choice its only case, and what it is
/// about. The choice is what `REGELHET_CASUS` is to the server.
///
/// # Errors
///
/// When the manifest does not make that case, or without a choice not
/// exactly one, when the case does not load, or when the server does not
/// start on it in test mode.
pub async fn check(manifest: &Path, chosen: Option<&str>) -> Result<Summary, CheckError> {
    let (corpus, messages) = case::load_chosen(manifest, chosen)?;
    let manifest = manifest.to_string_lossy().into_owned();
    let config = Config::from_env(|key| match key {
        "REGELHET_MODE" => Some("test".to_string()),
        "REGELHET_MANIFEST" => Some(manifest.clone()),
        "REGELHET_CASUS" => chosen.map(str::to_string),
        _ => None,
    })?;
    // Building the router is the check; nothing serves it.
    let _router = app::build_with(&config, Login::TestLogin).await?;
    let casus = &corpus.casus;
    Ok(Summary {
        cells: corpus.cellen().map(str::to_string).collect(),
        id: casus.id.clone(),
        title: casus.titel.clone(),
        executor: casus.cel.clone(),
        executor_name: casus.cel_naam.clone(),
        article: casus.artikel.clone(),
        outcome: casus.uitkomst.clone(),
        start_date: casus.speeldatum,
        messages,
    })
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "case:       {} ({})", self.id, self.title)?;
        writeln!(f, "executor:   {} ({})", self.executor, self.executor_name)?;
        writeln!(f, "article:    {}", self.article)?;
        writeln!(f, "outcome:    {}", self.outcome)?;
        writeln!(f, "cells:      {}", self.cells.join(", "))?;
        writeln!(f, "start date: {}", self.start_date)?;
        for message in &self.messages {
            writeln!(f, "note:       {message}")?;
        }
        Ok(())
    }
}
