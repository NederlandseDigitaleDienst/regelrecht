use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A corpus source definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Unique identifier for this source.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Source type and configuration.
    #[serde(flatten)]
    pub source_type: SourceType,
    /// Jurisdictional scopes this source is allowed to provide.
    /// Empty means unrestricted.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// Priority value. Lower value = higher priority.
    /// The central corpus uses priority 1.
    pub priority: u32,
    /// Reference to auth configuration. When absent, source is public.
    #[serde(default)]
    pub auth_ref: Option<String>,
    /// When set, token resolution for this source never falls back to the
    /// legacy shared `CORPUS_GIT_TOKEN` (see
    /// [`crate::auth::TokenContext::strict`]). Set for sources whose
    /// `auth_ref` derives from user input — a traject's writable-own repo —
    /// where the legacy fallback would ship the central token to a
    /// user-chosen repo on every read/scan. Not part of the manifest
    /// format; operator-managed manifest sources default to `false`.
    #[serde(default)]
    pub strict_auth: bool,
    /// Whether this source's laws can carry `machine_readable` content.
    /// `Some(false)` says they never do: the editor then reports none of them
    /// as enriched without reading their bodies (the central corpus, which is
    /// text only on purpose and too large to scan). Absent, each law is
    /// judged by its own body once that body is loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enriched: Option<bool>,
    /// Load every law of this source into the public library, not only the
    /// listed ones (`favorites.json`). Meant for a small source, such as a
    /// repository of example rulework. Trajecten index every source anyway.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub load_all: bool,
}

/// Source type discriminator with configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SourceType {
    Local {
        local: LocalSource,
    },
    #[serde(rename = "github")]
    GitHub {
        github: GitHubSource,
    },
}

/// Configuration for a local filesystem source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSource {
    /// Path to the regulation directory (relative to project root or absolute).
    pub path: PathBuf,
}

/// Configuration for a GitHub repository source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubSource {
    /// GitHub repository owner (organization or user).
    pub owner: String,
    /// GitHub repository name.
    pub repo: String,
    /// Branch to read from.
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Path within the repository to the regulation directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Optional Git ref (tag, branch, or commit SHA) for pinning.
    /// When set, overrides `branch` for fetching.
    #[serde(default, rename = "ref")]
    pub git_ref: Option<String>,
}

impl GitHubSource {
    /// Returns the `owner/repo` string for GitHub API calls.
    pub fn full_repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Returns the effective ref to fetch: `git_ref` if set, otherwise `branch`.
    pub fn effective_ref(&self) -> &str {
        self.git_ref.as_deref().unwrap_or(&self.branch)
    }
}

fn default_branch() -> String {
    "main".to_string()
}

/// Jurisdictional scope definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    /// Scope type (e.g., "gemeente_code", "provincie_code", "waterschap_code").
    #[serde(rename = "type")]
    pub scope_type: String,
    /// Scope value (e.g., "GM0363" for Amsterdam).
    pub value: String,
}

/// Top-level corpus registry manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryManifest {
    /// Schema version for forward compatibility.
    pub schema_version: String,
    /// List of corpus sources.
    pub sources: Vec<Source>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_local_source() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "MinBZK Central Corpus"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
"#;
        let manifest: RegistryManifest = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(manifest.schema_version, "1.0");
        assert_eq!(manifest.sources.len(), 1);

        let source = &manifest.sources[0];
        assert_eq!(source.id, "central");
        assert_eq!(source.priority, 1);
        match &source.source_type {
            SourceType::Local { local } => {
                assert_eq!(local.path, PathBuf::from("corpus/regulation/nl"));
            }
            _ => panic!("Expected local source type"),
        }
    }

    #[test]
    fn test_deserialize_github_source() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      owner: gemeente-amsterdam
      repo: regelrecht-amsterdam
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: GM0363
    priority: 10
    auth_ref: amsterdam
"#;
        let manifest: RegistryManifest = serde_yaml_ng::from_str(yaml).unwrap();
        let source = &manifest.sources[0];
        assert_eq!(source.id, "amsterdam");
        assert_eq!(source.priority, 10);
        assert_eq!(source.scopes.len(), 1);
        assert_eq!(source.scopes[0].value, "GM0363");
        assert_eq!(source.auth_ref.as_deref(), Some("amsterdam"));

        match &source.source_type {
            SourceType::GitHub { github } => {
                assert_eq!(github.owner, "gemeente-amsterdam");
                assert_eq!(github.repo, "regelrecht-amsterdam");
                assert_eq!(
                    github.full_repo(),
                    "gemeente-amsterdam/regelrecht-amsterdam"
                );
                assert_eq!(github.branch, "main");
                assert_eq!(github.path, Some("regulation/nl".to_string()));
                assert_eq!(github.effective_ref(), "main"); // no ref set, falls back to branch
            }
            _ => panic!("Expected GitHub source type"),
        }
    }

    #[test]
    fn test_github_source_with_ref() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: pinned
    name: "Pinned Source"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      ref: v2025.1
    priority: 1
"#;
        let manifest: RegistryManifest = serde_yaml_ng::from_str(yaml).unwrap();
        match &manifest.sources[0].source_type {
            SourceType::GitHub { github } => {
                assert_eq!(github.git_ref.as_deref(), Some("v2025.1"));
                assert_eq!(github.effective_ref(), "v2025.1");
            }
            _ => panic!("Expected GitHub source type"),
        }
    }

    #[test]
    fn test_serialize_roundtrip() {
        let manifest = RegistryManifest {
            schema_version: "1.0".to_string(),
            sources: vec![Source {
                id: "test".to_string(),
                name: "Test Source".to_string(),
                source_type: SourceType::Local {
                    local: LocalSource {
                        path: PathBuf::from("test/path"),
                    },
                },
                scopes: vec![],
                priority: 5,
                auth_ref: None,
                strict_auth: false,
                enriched: None,
                load_all: false,
            }],
        };

        let yaml = serde_yaml_ng::to_string(&manifest).unwrap();
        let parsed: RegistryManifest = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.sources[0].id, "test");
        assert_eq!(parsed.sources[0].priority, 5);
    }
}
