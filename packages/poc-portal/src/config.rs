//! Startup configuration, read from the environment the platform injects.
//!
//! Everything here fails loud. A portal that starts with half its
//! configuration is a portal that serves a PoC to whoever asks, and that is
//! the one failure this crate exists to prevent.

use std::collections::HashMap;

use crate::gate::Sleutel;
use crate::registry::{Poc, Registry};

pub struct Config {
    pub registry: Registry,
    pub sleutel: Sleutel,
    /// Password per slug, resolved at startup from `POC_PW_<SLUG>`.
    pub wachtwoorden: HashMap<String, String>,
    /// Root of the built static PoCs; each `statisch` PoC lives in
    /// `{static_root}/{slug}`.
    pub static_root: String,
    pub port: u16,
    /// Deployment name, used to address a proxied PoC in-cluster as
    /// `http://{deployment}-{upstream}:8000`. `None` outside the cluster,
    /// where `POC_UPSTREAM_<SLUG>` takes over.
    pub deployment: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("POC_COOKIE_SECRET is not set — without it no cookie can be signed")]
    NoSecret,
    #[error("POC_COOKIE_SECRET: {0}")]
    BadSecret(&'static str),
    #[error(
        "no password for PoC {slug:?}: set {env}. A PoC in the register with no password would \
         otherwise be served to anyone, so the portal refuses to start instead."
    )]
    NoPassword { slug: String, env: String },
    #[error("registry: {0}")]
    Registry(String),
}

/// Where to reach a proxied PoC.
///
/// In the cluster the pod's own `HOSTNAME` says which deployment we are in —
/// the same signal `editor-api` uses to find `pipelineapi`
/// (`packages/editor-api/src/main.rs`, `resolve_pipeline_api_url`). It is
/// preferred over an environment variable because ZAD's `clone-from` copies
/// env vars from production into a preview, and a preview must talk to its own
/// napp rather than production's. `POC_UPSTREAM_<SLUG>` remains for local dev,
/// where the hostname has no such shape.
pub fn upstream_url(
    deployment: Option<&str>,
    poc: &Poc,
    env_override: Option<String>,
) -> Option<String> {
    if let Some(url) = env_override {
        return Some(url);
    }
    let upstream = poc.upstream.as_deref()?;
    let deployment = deployment?;
    Some(format!("http://{deployment}-{upstream}:8000"))
}

impl Config {
    /// Point this configuration at another static root.
    ///
    /// For tests: `POC_STATIC_DIR` is process-wide, so a test that sets it
    /// races every other test in the binary — which is how a passing suite
    /// still failed one run in five. Building the config and then moving it
    /// keeps that choice on the test rather than on the environment.
    #[must_use]
    pub fn met_static_root(mut self, root: impl Into<String>) -> Self {
        self.static_root = root.into();
        self
    }

    /// Read the configuration, or explain what is missing.
    pub fn from_env(registry_yaml: &str) -> Result<Self, ConfigError> {
        let registry =
            Registry::from_yaml(registry_yaml).map_err(|e| ConfigError::Registry(e.to_string()))?;

        let secret = std::env::var("POC_COOKIE_SECRET").map_err(|_| ConfigError::NoSecret)?;
        let sleutel = Sleutel::new(&secret).map_err(ConfigError::BadSecret)?;

        let mut wachtwoorden = HashMap::new();
        for poc in &registry.pocs {
            let env = poc.wachtwoord_env();
            let value = std::env::var(&env)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| ConfigError::NoPassword {
                    slug: poc.slug.clone(),
                    env: env.clone(),
                })?;
            wachtwoorden.insert(poc.slug.clone(), value);
        }

        Ok(Self {
            registry,
            sleutel,
            wachtwoorden,
            static_root: std::env::var("POC_STATIC_DIR")
                .unwrap_or_else(|_| "/app/static".to_string()),
            port: std::env::var("POC_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8000),
            deployment: std::env::var("HOSTNAME")
                .ok()
                .as_deref()
                .and_then(regelrecht_corpus::deployment_from_hostname),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::registry::Soort;

    fn proxy_poc() -> Poc {
        Poc {
            slug: "napp".into(),
            titel: "N".into(),
            samenvatting: "S".into(),
            titel_intern: None,
            samenvatting_intern: None,
            soort: Soort::Proxy,
            status: crate::registry::Status::Verkenning,
            voorbehoud: "Een demonstratie, geen geldend recht.".into(),
            strook: true,
            bron: None,
            upstream: Some("napp".into()),
            corpus: vec![],
            assistent: false,
            tags: vec![],
            sources: vec![],
        }
    }

    #[test]
    fn the_upstream_follows_the_deployment_we_run_in() {
        assert_eq!(
            upstream_url(Some("pr123"), &proxy_poc(), None).as_deref(),
            Some("http://pr123-napp:8000"),
        );
        assert_eq!(
            upstream_url(Some("regelrecht"), &proxy_poc(), None).as_deref(),
            Some("http://regelrecht-napp:8000"),
        );
    }

    #[test]
    fn an_explicit_override_wins_for_local_dev() {
        assert_eq!(
            upstream_url(
                Some("regelrecht"),
                &proxy_poc(),
                Some("http://localhost:8400".into())
            )
            .as_deref(),
            Some("http://localhost:8400"),
        );
    }

    #[test]
    fn without_a_deployment_or_override_there_is_no_upstream() {
        // Better no URL than a guessed one: the proxy then answers 503 rather
        // than forwarding somewhere arbitrary.
        assert_eq!(upstream_url(None, &proxy_poc(), None), None);
    }
}
