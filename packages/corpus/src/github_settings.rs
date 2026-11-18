//! How the corpus reaches GitHub: the API base, the `corpus-auth.yaml` file
//! and the variables that carry server tokens.
//!
//! A server reads these once at startup with [`GithubSettings::from_env`] and
//! hands the value to every place that builds a GitHub client or resolves a
//! token. A test builds one with [`GithubSettings::new`] from its own values,
//! such as the address of a mock server, so it never has to change the process
//! environment.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::auth::CredentialResolver;

/// Reads a variable; the process environment outside tests.
type Lookup = dyn Fn(&str) -> Option<String> + Send + Sync;

/// The GitHub settings of one server. Cheap to clone.
#[derive(Clone)]
pub struct GithubSettings {
    /// The raw `GITHUB_API_BASE` value; `None` means `api.github.com`.
    api_base: Option<String>,
    auth_file: Option<PathBuf>,
    env: Arc<Lookup>,
    /// Repository tree listings, shared by every clone of these settings:
    /// the global corpus, each traject and each index refresh.
    #[cfg(feature = "github")]
    tree_listings: Arc<crate::github::TreeListings>,
}

impl fmt::Debug for GithubSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GithubSettings")
            .field("api_base", &self.api_base)
            .field("auth_file", &self.auth_file)
            .finish_non_exhaustive()
    }
}

impl GithubSettings {
    /// Settings from the process environment: `GITHUB_API_BASE` is read now,
    /// the token variables each time a token is resolved.
    pub fn from_env(auth_file: Option<PathBuf>) -> Self {
        Self {
            api_base: std::env::var("GITHUB_API_BASE").ok(),
            auth_file,
            env: Arc::new(crate::auth::process_env),
            #[cfg(feature = "github")]
            tree_listings: Arc::default(),
        }
    }

    /// Settings from explicit values, with the token variables read through
    /// `env`.
    pub fn new(
        api_base: Option<&str>,
        auth_file: Option<PathBuf>,
        env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            api_base: api_base.map(str::to_string),
            auth_file,
            env: Arc::new(env),
            #[cfg(feature = "github")]
            tree_listings: Arc::default(),
        }
    }

    /// Settings with no GitHub override and no tokens: `api.github.com`, no
    /// auth file, and every token variable unset. For tests and code paths
    /// that never reach GitHub.
    pub fn none() -> Self {
        Self::new(None, None, |_| None)
    }

    /// The `corpus-auth.yaml` file, if one is configured.
    pub fn auth_file(&self) -> Option<&Path> {
        self.auth_file.as_deref()
    }

    /// The resolver for server tokens under these settings.
    pub fn credentials(&self) -> CredentialResolver<'_> {
        CredentialResolver::with_env(self.auth_file.as_deref(), &*self.env)
    }

    /// The tree listings shared by everything built from these settings.
    #[cfg(feature = "github")]
    pub fn tree_listings(&self) -> &crate::github::TreeListings {
        &self.tree_listings
    }

    /// A GitHub client pointed at the configured API base.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    #[cfg(feature = "github")]
    pub fn client(&self) -> regelrecht_github::Result<regelrecht_github::GithubClient> {
        regelrecht_github::GithubClient::from_base(self.api_base.as_deref())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::auth::TokenContext;

    #[test]
    fn credentials_read_tokens_through_the_given_lookup() {
        let settings = GithubSettings::new(None, None, |key| {
            (key == "CORPUS_AUTH_SETTINGS_SRC_TOKEN").then(|| "from-lookup".to_string())
        });
        let decision = settings
            .credentials()
            .resolve(TokenContext::strict("settings-src"))
            .unwrap();
        assert_eq!(decision.token(), Some("from-lookup"));
    }
}
