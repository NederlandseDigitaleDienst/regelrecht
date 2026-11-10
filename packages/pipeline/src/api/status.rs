use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::ApiState;

/// Maximum number of IDs per status query.
const MAX_STATUS_IDS: usize = 20;

#[derive(Deserialize)]
pub struct HarvestStatusQuery {
    pub ids: String,
}

#[derive(Serialize)]
pub struct HarvestStatusResponse {
    pub results: Vec<HarvestStatusEntry>,
}

#[derive(Serialize)]
pub struct HarvestStatusEntry {
    pub bwb_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

/// GET /harvest/status?ids=BWBR0001234,BWBR0005678
///
/// Returns the current pipeline status and slug (if known) for each BWB ID.
/// The frontend polls this endpoint after requesting a harvest to track progress.
pub async fn harvest_status(
    State(state): State<ApiState>,
    Query(query): Query<HarvestStatusQuery>,
) -> Result<Json<HarvestStatusResponse>, (StatusCode, String)> {
    let ids: Vec<String> = query
        .ids
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if ids.is_empty() {
        return Ok(Json(HarvestStatusResponse {
            results: Vec::new(),
        }));
    }
    if ids
        .iter()
        .any(|id| !id.starts_with("BWBR") || id.len() > 20)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "each id must start with 'BWBR' and be at most 20 characters".to_string(),
        ));
    }
    if ids.len() > MAX_STATUS_IDS {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("too many ids: maximum is {MAX_STATUS_IDS}"),
        ));
    }

    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT law_id, status::text, slug FROM law_entries WHERE law_id = ANY($1)")
            .bind(&ids)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to query harvest status");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to query harvest status".to_string(),
                )
            })?;

    let results = rows
        .into_iter()
        .map(|(law_id, status, slug)| HarvestStatusEntry {
            bwb_id: law_id,
            status,
            slug,
        })
        .collect();

    Ok(Json(HarvestStatusResponse { results }))
}
