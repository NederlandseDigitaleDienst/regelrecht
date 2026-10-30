use std::path::Path;

use crate::error::{CorpusError, Result};
use crate::models::{RegistryManifest, Source, SourceType};
use crate::source_map::SourceMap;

/// A source that failed to enumerate during
/// [`CorpusRegistry::index_all_sources_async`], with the error it failed on.
/// Carrying the message (not just the id) lets callers surface *why* the
/// source's laws are missing — a `law_count: 0` with no reason is
/// indistinguishable from a genuinely empty repo.
#[derive(Debug, Clone)]
pub struct SourceIndexFailure {
    pub source_id: String,
    pub error: String,
}

/// Per-call token override for the index scan of exactly **one** source.
///
/// Carries the acting user's personal GitHub token into the enumeration of
/// a traject's writable-own repo when the server has no `CORPUS_AUTH_*`
/// token for it — the scan-side mirror of the request-bound reads that
/// already fall back to the user's token. Two hard rules keep the token
/// contained:
///
/// * it is applied **only** when the server-side resolution for
///   `source_id` yields no token (a source with its own service token
///   keeps scanning with that), and never for any other source;
/// * it lives for the duration of the call — the registry never stores it.
#[derive(Clone, Copy)]
pub struct ScanTokenOverride<'a> {
    /// The one source id the override may authenticate.
    pub source_id: &'a str,
    /// The per-call token (e.g. the linked user's OAuth token).
    pub token: &'a str,
}

/// Corpus registry that manages source definitions.
///
/// Loads sources from `corpus-registry.yaml` and optionally merges
/// local overrides from `corpus-registry.local.yaml`.
#[derive(Debug, Clone)]
pub struct CorpusRegistry {
    sources: Vec<Source>,
}

impl CorpusRegistry {
    /// Create an empty registry (for tests that don't need corpus).
    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Load the registry from a manifest file, optionally merging a local override.
    ///
    /// The local override file replaces sources with the same `id` entirely.
    pub fn load(manifest_path: &Path, local_override_path: Option<&Path>) -> Result<Self> {
        let content = std::fs::read_to_string(manifest_path).map_err(|e| {
            CorpusError::Config(format!(
                "Failed to read registry manifest {}: {}",
                manifest_path.display(),
                e
            ))
        })?;

        let manifest: RegistryManifest = serde_yaml_ng::from_str(&content).map_err(|e| {
            CorpusError::Config(format!(
                "Failed to parse registry manifest {}: {}",
                manifest_path.display(),
                e
            ))
        })?;

        let mut sources = manifest.sources;

        if let Some(local_path) = local_override_path {
            if local_path.exists() {
                let local_content = std::fs::read_to_string(local_path).map_err(|e| {
                    CorpusError::Config(format!(
                        "Failed to read local override {}: {}",
                        local_path.display(),
                        e
                    ))
                })?;

                let local_manifest: RegistryManifest = serde_yaml_ng::from_str(&local_content)
                    .map_err(|e| {
                        CorpusError::Config(format!(
                            "Failed to parse local override {}: {}",
                            local_path.display(),
                            e
                        ))
                    })?;

                sources = merge_sources(sources, local_manifest.sources);
            }
        }

        // Sort by priority (lowest value = highest priority)
        sources.sort_by_key(|s| s.priority);

        Ok(Self { sources })
    }

    /// Build a registry from an in-memory list of sources.
    ///
    /// Used by the trajects layer to construct a per-traject registry from
    /// rows stored in `traject_corpus_sources` — no YAML file involved.
    /// Sources are sorted by priority on construction so the rest of the
    /// API behaves identically to a yaml-loaded registry.
    pub fn from_sources(mut sources: Vec<Source>) -> Self {
        sources.sort_by_key(|s| s.priority);
        Self { sources }
    }

    /// Load from a YAML string (useful for testing).
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let manifest: RegistryManifest = serde_yaml_ng::from_str(yaml)
            .map_err(|e| CorpusError::Config(format!("Failed to parse registry YAML: {}", e)))?;

        let mut sources = manifest.sources;
        sources.sort_by_key(|s| s.priority);

        Ok(Self { sources })
    }

    /// Get all sources, ordered by priority (lowest value first).
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Get a source by ID.
    pub fn get_source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Load all local sources into a SourceMap.
    ///
    /// GitHub sources are skipped — use [`index_all_sources_async`] or
    /// [`load_favorites_async`] to include them.
    ///
    /// `today` is the date a law's versions are collapsed against; see
    /// [`SourceMap::new`] for why it is passed in rather than read off a clock.
    pub fn load_local_sources(&self, today: &str) -> Result<SourceMap> {
        let mut map = SourceMap::new(today);
        for source in &self.sources {
            match &source.source_type {
                SourceType::Local { .. } => {
                    map.load_source(source)?;
                }
                SourceType::GitHub { .. } => {
                    tracing::debug!(
                        source_id = %source.id,
                        "Skipping GitHub source in sync load"
                    );
                }
            }
        }

        // Validate scopes and log warnings
        let warnings = crate::validation::validate_scopes(&map, &self.sources);
        for w in &warnings {
            tracing::warn!(
                law_id = %w.law_id,
                source_id = %w.source_id,
                "{}",
                w.message
            );
        }

        Ok(map)
    }

    /// Load local sources + only the specified laws from GitHub sources.
    ///
    /// Uses the Trees API (1 call per GitHub source) to discover paths,
    /// then fetches only the files matching `law_ids`. This keeps startup
    /// fast and avoids burning rate limits on thousands of unused files.
    /// A source marked `load_all` is the exception: it is small by design,
    /// so every law in it is fetched.
    #[cfg(feature = "github")]
    pub async fn load_favorites_async(
        &self,
        law_ids: &std::collections::HashSet<String>,
        auth_file: Option<&Path>,
        today: &str,
    ) -> Result<SourceMap> {
        let mut map = SourceMap::new(today);
        let client = regelrecht_github::GithubClient::new()?;

        // Determine which law_ids are NOT already covered by local sources,
        // so we only fetch what's missing from GitHub.
        for source in &self.sources {
            if let SourceType::Local { .. } = &source.source_type {
                map.load_source(source)?;
            }
        }
        let local_ids: std::collections::HashSet<String> =
            map.laws().map(|l| l.law_id.clone()).collect();
        let missing: std::collections::HashSet<String> =
            law_ids.difference(&local_ids).cloned().collect();
        let any_load_all = self
            .sources
            .iter()
            .any(|s| s.load_all && matches!(s.source_type, SourceType::GitHub { .. }));

        if missing.is_empty() && !any_load_all {
            tracing::info!("all favorites available locally, skipping GitHub fetch");
            return Ok(map);
        }

        for source in &self.sources {
            if let SourceType::GitHub { github } = &source.source_type {
                // A `load_all` source is fetched whole; any other only for the
                // listed laws that no local source already provides.
                let filter = if source.load_all {
                    None
                } else {
                    Some(&missing)
                };
                let token = crate::auth::CredentialResolver::new(auth_file)
                    .resolve_source(source)?
                    .into_token();
                match crate::github::fetch_source_filtered(
                    &client,
                    github,
                    token.as_deref(),
                    filter,
                    today,
                )
                .await?
                {
                    crate::github::FetchResult::Fetched(files) => {
                        for file in &files {
                            map.load_fetched_file(
                                &file.content,
                                &file.path,
                                github.path.as_deref(),
                                &source.id,
                                &source.name,
                                source.priority,
                            )?;
                        }
                    }
                    crate::github::FetchResult::NotModified => {}
                }
            }
        }

        // Validate scopes and log warnings
        let warnings = crate::validation::validate_scopes(&map, &self.sources);
        for w in &warnings {
            tracing::warn!(
                law_id = %w.law_id,
                source_id = %w.source_id,
                "{}",
                w.message
            );
        }

        Ok(map)
    }

    /// Build a lightweight **index** of every law across all sources without
    /// fetching law bodies. Local sources are loaded eagerly (disk is cheap
    /// and gives real names); GitHub sources are enumerated via the Trees API
    /// (1 call per source) into metadata-only entries — `law_id`, source, and
    /// `relative_path`, but no content. Bodies are fetched lazily on first
    /// read via the source's backend.
    ///
    /// This replaces the eager [`load_all_sources_async`] on the traject path:
    /// loading every law's full YAML up front meant N per-file Contents API
    /// calls per traject build, which is slow and trips GitHub's secondary
    /// rate limit. The library/search only needs the index; content is only
    /// needed when a specific law is opened.
    ///
    /// Returns the index plus a [`SourceIndexFailure`] per source that failed
    /// to enumerate (non-fatal, mirroring [`load_all_sources_async`]) — the
    /// error string travels with the id so callers can surface *why* a
    /// source's laws are missing instead of only showing a zero law count.
    #[cfg(feature = "github")]
    pub async fn index_all_sources_async(
        &self,
        auth_file: Option<&Path>,
        today: &str,
    ) -> Result<(SourceMap, Vec<SourceIndexFailure>)> {
        self.index_all_sources_with_override(auth_file, None, today)
            .await
    }

    /// [`Self::index_all_sources_async`] with an optional per-call
    /// [`ScanTokenOverride`]: the traject build path passes the acting
    /// user's token here so the writable-own repo can be enumerated when
    /// the server has no token for it. See the override type for the
    /// containment rules (single source, server token wins, never stored).
    #[cfg(feature = "github")]
    pub async fn index_all_sources_with_override(
        &self,
        auth_file: Option<&Path>,
        scan_override: Option<ScanTokenOverride<'_>>,
        today: &str,
    ) -> Result<(SourceMap, Vec<SourceIndexFailure>)> {
        let mut map = SourceMap::new(today);
        let client = regelrecht_github::GithubClient::new()?;
        let mut failed: Vec<SourceIndexFailure> = Vec::new();

        for source in &self.sources {
            if let Err(e) =
                Self::index_one_source(&mut map, &client, source, auth_file, scan_override).await
            {
                tracing::warn!(
                    source_id = %source.id,
                    error = %e,
                    "failed to index corpus source, skipping"
                );
                failed.push(SourceIndexFailure {
                    source_id: source.id.clone(),
                    error: e.to_string(),
                });
            }
        }

        if !failed.is_empty() {
            tracing::warn!(
                failed = ?failed.iter().map(|f| f.source_id.as_str()).collect::<Vec<_>>(),
                indexed = map.len(),
                "some corpus sources failed to index"
            );
        }

        for w in crate::validation::validate_scopes(&map, &self.sources) {
            tracing::warn!(
                law_id = %w.law_id,
                source_id = %w.source_id,
                "{}",
                w.message
            );
        }

        Ok((map, failed))
    }

    /// Index a single source: load local content eagerly, but enumerate a
    /// GitHub source's laws by path only (no body fetch). Factored out so one
    /// source's failure is skipped, not fatal.
    #[cfg(feature = "github")]
    async fn index_one_source(
        map: &mut SourceMap,
        client: &regelrecht_github::GithubClient,
        source: &Source,
        auth_file: Option<&Path>,
        scan_override: Option<ScanTokenOverride<'_>>,
    ) -> Result<()> {
        match &source.source_type {
            SourceType::Local { .. } => {
                map.load_source(source)?;
            }
            SourceType::GitHub { github } => {
                // `resolve_source` honours `strict_auth`: the scan of a
                // traject's writable-own repo resolves with exactly the same
                // rules as its push path, so a repo the server can push to is
                // also a repo the server can index (and vice versa — no more
                // "promote succeeded but the index reads with a different
                // token and comes back empty").
                let mut token = crate::auth::CredentialResolver::new(auth_file)
                    .resolve_source(source)?
                    .into_token();
                // Only when the server resolves NO token may the per-call
                // override authenticate this scan — and only for the one
                // source it was issued for. A configured service token always
                // wins (mirroring the request-bound read fallback), and the
                // override never reaches any other (seed) source.
                if token.is_none() {
                    if let Some(o) = scan_override.filter(|o| o.source_id == source.id) {
                        token = Some(o.token.to_string());
                    }
                }
                let today = map.reference_date().to_string();
                for (law_id, path, sha) in
                    crate::github::list_source_law_paths(client, github, token.as_deref(), &today)
                        .await?
                {
                    map.load_metadata_entry(
                        &law_id,
                        &path,
                        github.path.as_deref(),
                        &source.id,
                        &source.name,
                        source.priority,
                        sha.as_deref(),
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl CorpusRegistry {
    /// Add every law of each GitHub source to `map` by path only (no body),
    /// for the laws `map` does not hold yet. The editor's public library loads
    /// the bodies of a few laws up front ([`Self::load_favorites_async`]) and
    /// uses this to make the rest of each source findable; a body is fetched
    /// when someone opens the law.
    ///
    /// Skipped, so what was loaded stays as it is: a source marked `load_all`
    /// (already loaded whole), a law the same source already supplied, and a
    /// law a local source provides (a local copy hides the GitHub versions of
    /// that law, as in [`Self::load_favorites_async`]). A law the map refuses
    /// (two sources at equal priority) is logged and left out rather than
    /// failing the source; call this after [`SourceMap::prefer_version_in_force`],
    /// whose insert refuses such a law before recording it as a version. Returns a [`SourceIndexFailure`] per source that
    /// could not be enumerated.
    #[cfg(feature = "github")]
    pub async fn index_github_sources_into(
        &self,
        map: &mut SourceMap,
        auth_file: Option<&Path>,
    ) -> Vec<SourceIndexFailure> {
        let mut failed = Vec::new();
        let client = match regelrecht_github::GithubClient::new() {
            Ok(client) => client,
            Err(e) => {
                for source in &self.sources {
                    if matches!(source.source_type, SourceType::GitHub { .. }) && !source.load_all {
                        failed.push(SourceIndexFailure {
                            source_id: source.id.clone(),
                            error: e.to_string(),
                        });
                    }
                }
                return failed;
            }
        };
        let local_ids: std::collections::HashSet<&str> = self
            .sources
            .iter()
            .filter(|s| matches!(s.source_type, SourceType::Local { .. }))
            .map(|s| s.id.as_str())
            .collect();
        let today = map.reference_date().to_string();

        for source in &self.sources {
            let SourceType::GitHub { github } = &source.source_type else {
                continue;
            };
            if source.load_all {
                continue;
            }
            let listed = async {
                let token = crate::auth::CredentialResolver::new(auth_file)
                    .resolve_source(source)?
                    .into_token();
                crate::github::list_source_law_paths(&client, github, token.as_deref(), &today)
                    .await
            }
            .await;
            let paths = match listed {
                Ok(paths) => paths,
                Err(e) => {
                    tracing::warn!(source_id = %source.id, error = %e, "failed to index corpus source, skipping");
                    failed.push(SourceIndexFailure {
                        source_id: source.id.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            let mut added = 0usize;
            for (law_id, path, sha) in paths {
                let held = map.get_law_versions(&law_id).is_some_and(|versions| {
                    versions.iter().any(|v| {
                        v.source_id == source.id || local_ids.contains(v.source_id.as_str())
                    })
                });
                if held {
                    continue;
                }
                match map.load_metadata_entry(
                    &law_id,
                    &path,
                    github.path.as_deref(),
                    &source.id,
                    &source.name,
                    source.priority,
                    sha.as_deref(),
                ) {
                    Ok(()) => added += 1,
                    Err(e) => {
                        tracing::warn!(source_id = %source.id, law_id = %law_id, error = %e, "law left out of the index");
                    }
                }
            }
            tracing::info!(source_id = %source.id, added, "indexed corpus source by path");
        }
        failed
    }
}

/// Merge base sources with local overrides.
///
/// Sources in `overrides` with the same `id` as a base source replace it entirely.
/// Sources in `overrides` with new `id`s are appended.
fn merge_sources(base: Vec<Source>, overrides: Vec<Source>) -> Vec<Source> {
    let mut result = base;

    for override_source in overrides {
        if let Some(pos) = result.iter().position(|s| s.id == override_source.id) {
            result[pos] = override_source;
        } else {
            result.push(override_source);
        }
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::SourceType;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_yaml(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_load_manifest() {
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
        let file = write_temp_yaml(yaml);
        let registry = CorpusRegistry::load(file.path(), None).unwrap();

        assert_eq!(registry.sources().len(), 1);
        assert_eq!(registry.sources()[0].id, "central");
        assert_eq!(registry.sources()[0].priority, 1);
    }

    #[test]
    fn test_load_with_local_override() {
        let base_yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "MinBZK Central Corpus"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: local
    local:
      path: /remote/amsterdam
    scopes: []
    priority: 10
"#;
        let override_yaml = r#"
schema_version: "1.0"
sources:
  - id: amsterdam
    name: "Amsterdam Local Dev"
    type: local
    local:
      path: /local/amsterdam
    scopes: []
    priority: 10
"#;
        let base_file = write_temp_yaml(base_yaml);
        let override_file = write_temp_yaml(override_yaml);

        let registry = CorpusRegistry::load(base_file.path(), Some(override_file.path())).unwrap();

        assert_eq!(registry.sources().len(), 2);

        let amsterdam = registry.get_source("amsterdam").unwrap();
        assert_eq!(amsterdam.name, "Amsterdam Local Dev");
        match &amsterdam.source_type {
            SourceType::Local { local } => {
                assert_eq!(local.path, std::path::PathBuf::from("/local/amsterdam"));
            }
            _ => panic!("Expected local source"),
        }
    }

    #[test]
    fn test_local_override_adds_new_source() {
        let base_yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: corpus/regulation/nl
    scopes: []
    priority: 1
"#;
        let override_yaml = r#"
schema_version: "1.0"
sources:
  - id: my-gemeente
    name: "My Gemeente"
    type: local
    local:
      path: /local/my-gemeente
    scopes: []
    priority: 20
"#;
        let base_file = write_temp_yaml(base_yaml);
        let override_file = write_temp_yaml(override_yaml);

        let registry = CorpusRegistry::load(base_file.path(), Some(override_file.path())).unwrap();

        assert_eq!(registry.sources().len(), 2);
        assert!(registry.get_source("my-gemeente").is_some());
    }

    #[test]
    fn test_sources_sorted_by_priority() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: low-prio
    name: "Low Priority"
    type: local
    local:
      path: /low
    scopes: []
    priority: 100
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
  - id: mid-prio
    name: "Mid Priority"
    type: local
    local:
      path: /mid
    scopes: []
    priority: 10
"#;
        let registry = CorpusRegistry::from_yaml(yaml).unwrap();
        let sources = registry.sources();

        assert_eq!(sources[0].id, "central");
        assert_eq!(sources[0].priority, 1);
        assert_eq!(sources[1].id, "mid-prio");
        assert_eq!(sources[1].priority, 10);
        assert_eq!(sources[2].id, "low-prio");
        assert_eq!(sources[2].priority, 100);
    }

    #[test]
    fn test_missing_local_override_is_ok() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
"#;
        let file = write_temp_yaml(yaml);
        let nonexistent = Path::new("/nonexistent/override.yaml");

        let registry = CorpusRegistry::load(file.path(), Some(nonexistent)).unwrap();
        assert_eq!(registry.sources().len(), 1);
    }

    #[test]
    fn test_invalid_yaml_returns_error() {
        let yaml = "not: [valid: yaml: {{{";
        let file = write_temp_yaml(yaml);

        let result = CorpusRegistry::load(file.path(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_source_by_id() {
        let yaml = r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: /central
    scopes: []
    priority: 1
  - id: amsterdam
    name: "Amsterdam"
    type: local
    local:
      path: /amsterdam
    scopes: []
    priority: 10
"#;
        let registry = CorpusRegistry::from_yaml(yaml).unwrap();

        assert!(registry.get_source("central").is_some());
        assert!(registry.get_source("amsterdam").is_some());
        assert!(registry.get_source("nonexistent").is_none());
    }
}
