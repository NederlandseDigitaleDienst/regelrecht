use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use regelrecht_corpus::dto::{PaginationParams, SourceSummary, build_source_summaries};
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

/// A law entry with source provenance.
#[derive(Debug, Serialize)]
pub struct CorpusLawEntry {
    pub law_id: String,
    pub source_id: String,
    pub source_name: String,
    pub file_path: String,
}

/// GET /api/sources — list all registered corpus sources with law counts.
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<SourceSummary>>, ApiError> {
    let corpus = state.corpus.read().await;
    // The admin corpus state doesn't track per-source scan health (its
    // sources are operator-managed manifest entries); pass no failures so
    // `index_error` serialises as null on every source.
    Ok(Json(build_source_summaries(
        &corpus.registry,
        &corpus.source_map,
        &std::collections::HashMap::new(),
    )))
}

/// GET /api/corpus/laws — list loaded laws with source metadata.
///
/// Supports pagination via `?offset=0&limit=100`. Default limit is 100,
/// maximum is 1000.
pub async fn list_corpus_laws(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CorpusLawEntry>>, ApiError> {
    let corpus = state.corpus.read().await;
    let limit = params.effective_limit();

    let mut entries: Vec<CorpusLawEntry> = corpus
        .source_map
        .laws()
        .map(|law| CorpusLawEntry {
            law_id: law.law_id.clone(),
            source_id: law.source_id.clone(),
            source_name: law.source_name.clone(),
            file_path: std::path::Path::new(&law.file_path)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| {
                    tracing::warn!(path = %law.file_path, "unexpected file_path with no filename");
                    String::new()
                }),
        })
        .collect();

    entries.sort_by(|a, b| a.law_id.cmp(&b.law_id));

    let paginated: Vec<CorpusLawEntry> = entries
        .into_iter()
        .skip(params.offset)
        .take(limit)
        .collect();

    Ok(Json(paginated))
}

/// POST /api/sources/{id}/sync — rebuild all local corpus sources.
///
/// Validates that the given source exists and is local, then does a
/// full rebuild of the source map from all local sources. This ensures
/// conflict resolution is correct: if a winning source drops a law,
/// the previously-shadowed version from another source gets promoted.
///
/// Disk I/O runs outside the write lock to avoid blocking reads.
pub async fn sync_source(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    // Phase 1: read lock — validate source exists and is local, clone registry for I/O
    let registry = {
        let corpus = state.corpus.read().await;

        let source = corpus
            .registry
            .get_source(&source_id)
            .ok_or_else(|| ApiError::NotFound(format!("Source '{}' not found", source_id)))?;

        if matches!(
            source.source_type,
            regelrecht_corpus::SourceType::GitHub { .. }
        ) {
            return Err(ApiError::NotImplemented(format!(
                "Source '{}' is a GitHub source — sync is not yet supported for remote sources",
                source_id
            )));
        }

        corpus.registry.clone()
    }; // read lock released

    // Phase 2: no lock held — do all disk I/O on a blocking thread
    // to avoid blocking the Tokio worker pool during directory traversal.
    let today = regelrecht_shared::dates::today_str();
    let new_map = tokio::task::spawn_blocking(move || registry.load_local_sources(&today))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "spawn_blocking task failed");
            ApiError::Internal("failed to reload sources".to_string())
        })?
        .map_err(|e| ApiError::Internal(format!("Failed to reload sources: {}", e)))?;

    // Compute counts before acquiring write lock to minimize lock duration.
    let source_law_count = new_map.laws().filter(|l| l.source_id == source_id).count();
    let total_law_count = new_map.len();

    // Phase 3: write lock — brief swap only
    let mut corpus = state.corpus.write().await;
    corpus.source_map = new_map;
    drop(corpus);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "source_id": source_id,
            "source_law_count": source_law_count,
            "total_law_count": total_law_count,
            "status": "synced"
        })),
    ))
}
