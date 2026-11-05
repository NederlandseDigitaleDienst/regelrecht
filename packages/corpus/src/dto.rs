//! Shared DTOs and helpers for corpus list endpoints.
//!
//! The admin and editor-api crates each expose `GET /api/sources` and
//! related endpoints. The body of those handlers — pagination
//! parameters, source-summary serialization, and the registry-walk that
//! produces the summaries — is identical between the two callers. This
//! module is the single source of truth so they can stay in sync.

use serde::{Deserialize, Serialize};

use crate::{CorpusRegistry, SourceMap, SourceType};

/// Default page size for list endpoints. Internal — callers go through
/// [`PaginationParams::effective_limit`].
const DEFAULT_LIMIT: usize = 100;
/// Maximum page size accepted on list endpoints. Internal — callers go
/// through [`PaginationParams::effective_limit`].
const MAX_LIMIT: usize = 1000;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional case-insensitive search term. When present, the law list is
    /// filtered server-side to laws whose `law_id` or name contains it —
    /// letting the editor search the full corpus index without shipping every
    /// law to the browser.
    #[serde(default)]
    pub q: Option<String>,
    /// Optional comma-separated set of exact `law_id`s. When present, only
    /// those laws are returned — so the library sidebar can resolve metadata
    /// for just the user's favorites and traject edits instead of fetching the
    /// whole corpus and filtering client-side.
    ///
    /// Takes precedence over [`q`](Self::q): if both are sent, `ids` wins and
    /// `q` is ignored (they're not combined). Callers pick one or the other.
    #[serde(default)]
    pub ids: Option<String>,
    /// Optional exact source id. When present, only laws provided by that
    /// source are returned — so the library sidebar can list the laws of a
    /// traject's own repo without also pulling in the federated central
    /// corpus.
    ///
    /// **Precedence.** `ids` and `q` both answer "*which* laws"; `source`
    /// answers "*whose* laws", which is orthogonal — so it does not compete
    /// with them, it narrows whichever of the two applies (logical AND):
    ///
    /// | sent | result |
    /// |---|---|
    /// | `source` | every law of that source |
    /// | `source` + `ids` | the `ids` that live in that source |
    /// | `source` + `q` | the `q` matches that live in that source |
    /// | `source` + `ids` + `q` | `ids` still beats `q`, then narrowed by source |
    ///
    /// An unknown source id matches nothing (empty list); an absent — or
    /// blank/whitespace-only — `source` filters nothing, leaving the
    /// pre-existing behaviour of every route that inherits this struct
    /// untouched.
    ///
    /// **`source` disables `offset` paging.** Like `ids` and `q` it selects the
    /// filtered branch, which returns the whole matching set capped at `limit`
    /// and ignores `offset`. A caller that pages the global route with
    /// `?source=…&offset=…` would otherwise keep receiving page one.
    #[serde(default)]
    pub source: Option<String>,
    /// Only laws whose current version is enriched (carries `machine_readable`
    /// and comes from a source not marked `enriched: false`). Narrows like
    /// [`source`](Self::source), but keeps `offset` paging when sent alone, so
    /// the library's overview can page through the enriched laws.
    #[serde(default)]
    pub enriched: bool,
    /// Only laws whose body is loaded in the index, leaving out the ones held
    /// by path until someone opens them. Those carry no name, so a caller that
    /// lists laws for their display names sends this and gets the few that
    /// have one, instead of a page of the whole corpus. Narrows like
    /// [`enriched`](Self::enriched).
    #[serde(default)]
    pub loaded: bool,
}

impl PaginationParams {
    /// Resolve the effective page size from the request, clamped to `MAX_LIMIT`.
    pub fn effective_limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
    }
}

/// Summary of a corpus source returned by `GET /api/sources`.
#[derive(Debug, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub priority: u32,
    pub law_count: usize,
    /// Why the last index scan of this source failed, when it did. `None`
    /// means the scan succeeded — so a `law_count` of 0 with `index_error:
    /// null` is a genuinely empty source, while `law_count: 0` plus an error
    /// here means the source couldn't be read at all (missing/invalid token,
    /// unreachable repo) and its laws are silently absent from resolution.
    pub index_error: Option<String>,
}

/// Stable string tag for a [`SourceType`] used in JSON responses.
pub fn source_type_tag(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::Local { .. } => "local",
        SourceType::GitHub { .. } => "github",
    }
}

/// Build the [`SourceSummary`] list returned by `GET /api/sources`.
///
/// `index_failures` maps source id → the error of the last failed index scan
/// (empty when every source enumerated fine, or when the caller doesn't
/// track scan health).
pub fn build_source_summaries(
    registry: &CorpusRegistry,
    source_map: &SourceMap,
    index_failures: &std::collections::HashMap<String, String>,
) -> Vec<SourceSummary> {
    registry
        .sources()
        .iter()
        .map(|source| {
            let law_count = source_map
                .laws()
                .filter(|law| law.source_id == source.id)
                .count();
            SourceSummary {
                id: source.id.clone(),
                name: source.name.clone(),
                source_type: source_type_tag(&source.source_type).to_string(),
                priority: source.priority,
                law_count,
                index_error: index_failures.get(&source.id).cloned(),
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::{LocalSource, Source};

    fn source(id: &str, priority: u32) -> Source {
        Source {
            id: id.to_string(),
            name: id.to_string(),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: std::path::PathBuf::from("unused"),
                },
            },
            scopes: vec![],
            priority,
            auth_ref: None,
            strict_auth: false,
            enriched: None,
            load_all: false,
            library_only: false,
        }
    }

    #[test]
    fn summaries_carry_the_index_error_of_a_failed_source() {
        // A failed scan and a genuinely empty source both show law_count 0;
        // `index_error` is what tells them apart on `GET /api/sources`.
        let registry =
            CorpusRegistry::from_sources(vec![source("broken", 0), source("healthy", 2)]);
        let failures = std::collections::HashMap::from([(
            "broken".to_string(),
            "GitHub Trees API returned 404".to_string(),
        )]);

        let summaries = build_source_summaries(&registry, &SourceMap::new("2026-06-01"), &failures);

        let broken = summaries.iter().find(|s| s.id == "broken").unwrap();
        assert_eq!(broken.law_count, 0);
        assert_eq!(
            broken.index_error.as_deref(),
            Some("GitHub Trees API returned 404")
        );
        let healthy = summaries.iter().find(|s| s.id == "healthy").unwrap();
        assert_eq!(healthy.index_error, None);
    }
}
