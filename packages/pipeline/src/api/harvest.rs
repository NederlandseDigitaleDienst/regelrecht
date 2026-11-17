use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::harvest_request::{self, HarvestRequestOptions, HarvestRequestOutcome};
use crate::models::Priority;

use crate::ApiState;

/// Priority for editor-requested harvest jobs (higher = processed first).
/// Default pipeline priority is 50, follow-up jobs use 30.
const EDITOR_HARVEST_PRIORITY: i32 = 80;

/// Maximum number of items per harvest request.
const MAX_HARVEST_IDS: usize = 100;

#[derive(Deserialize)]
pub struct HarvestRequest {
    /// Direct BWB ID (e.g. "BWBR0018451"). Used by BWB search UI.
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// Law slug (e.g. "participatiewet"). Used by dependency walker.
    #[serde(default)]
    pub law_id: Option<String>,
}

#[derive(Deserialize)]
pub struct HarvestBatchRequest {
    /// Law slugs to harvest (e.g. ["participatiewet", "zorgtoeslag"]).
    pub law_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct HarvestResponse {
    pub bwb_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

#[derive(Serialize)]
pub struct HarvestBatchResponse {
    pub results: Vec<HarvestResponse>,
}

/// POST /harvest
///
/// Unified harvest endpoint. Accepts either a `bwb_id` (from BWB search) or
/// a `law_id` (slug, from the dependency walker). Creates a high-priority
/// harvest job using the pipeline's existing job queue.
pub async fn request_harvest(
    State(state): State<ApiState>,
    Json(body): Json<HarvestRequest>,
) -> Result<Json<HarvestResponse>, (StatusCode, String)> {
    let (bwb_id, slug) = resolve_identifiers(&state, body.bwb_id, body.law_id).await?;

    let result = create_harvest_job(&state, &bwb_id, slug.as_deref()).await;
    Ok(Json(result))
}

/// POST /harvest/batch
///
/// Batch variant for the dependency walker. Accepts an array of law slugs
/// and creates harvest jobs for each.
pub async fn request_harvest_batch(
    State(state): State<ApiState>,
    Json(body): Json<HarvestBatchRequest>,
) -> Result<Json<HarvestBatchResponse>, (StatusCode, String)> {
    if body.law_ids.len() > MAX_HARVEST_IDS {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("too many law_ids: maximum is {MAX_HARVEST_IDS}"),
        ));
    }

    let mut results = Vec::with_capacity(body.law_ids.len());

    for slug in &body.law_ids {
        let slug = slug.trim();
        if slug.is_empty() || slug.len() > 256 {
            continue;
        }

        let result = match find_bwb_id_by_slug(&state.pool, slug).await {
            Ok(Some(bwb_id)) => create_harvest_job(&state, &bwb_id, Some(slug)).await,
            Ok(None) => HarvestResponse {
                bwb_id: slug.to_string(),
                status: "not_found".to_string(),
                slug: Some(slug.to_string()),
            },
            Err(e) => {
                tracing::error!(error = %e, slug = %slug, "failed to look up slug");
                HarvestResponse {
                    bwb_id: slug.to_string(),
                    status: "error".to_string(),
                    slug: Some(slug.to_string()),
                }
            }
        };

        results.push(result);
    }

    Ok(Json(HarvestBatchResponse { results }))
}

/// Resolve the BWB ID and optional slug from the request parameters.
async fn resolve_identifiers(
    state: &ApiState,
    bwb_id: Option<String>,
    law_id: Option<String>,
) -> Result<(String, Option<String>), (StatusCode, String)> {
    match (bwb_id, law_id) {
        // Direct BWB ID — used by BWB search UI
        (Some(bwb_id), _) => {
            let bwb_id = bwb_id.trim().to_string();
            if !bwb_id.starts_with("BWBR") || bwb_id.len() > 20 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "bwb_id must start with BWBR and be at most 20 characters".to_string(),
                ));
            }
            Ok((bwb_id, None))
        }
        // Slug — look up BWB ID from law_entries
        (None, Some(slug)) => {
            let slug = slug.trim().to_string();
            if slug.is_empty() || slug.len() > 256 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "law_id must be non-empty and at most 256 characters".to_string(),
                ));
            }
            match find_bwb_id_by_slug(&state.pool, &slug).await {
                Ok(Some(bwb_id)) => Ok((bwb_id, Some(slug))),
                Ok(None) => Err((
                    StatusCode::NOT_FOUND,
                    format!("no BWB ID mapping found for slug: {slug}"),
                )),
                Err(e) => {
                    tracing::error!(error = %e, slug = %slug, "failed to look up slug");
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to look up slug".to_string(),
                    ))
                }
            }
        }
        (None, None) => Err((
            StatusCode::BAD_REQUEST,
            "either bwb_id or law_id must be provided".to_string(),
        )),
    }
}

/// Find a law's BWB ID by its slug in the law_entries table.
pub async fn find_bwb_id_by_slug(
    pool: &sqlx::PgPool,
    slug: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT law_id FROM law_entries WHERE slug = $1 LIMIT 1")
            .bind(slug)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|(law_id,)| law_id))
}

/// Create a high-priority harvest job via the canonical pipeline entry point
/// ([`harvest_request::request_harvest`]) and map the outcome onto the
/// editor's status-string response shape.
async fn create_harvest_job(state: &ApiState, bwb_id: &str, slug: Option<&str>) -> HarvestResponse {
    let opts = HarvestRequestOptions {
        priority: Priority::new(EDITOR_HARVEST_PRIORITY),
        // Editor harvests always want the latest consolidation.
        date: None,
        law_name: None,
        slug: slug.map(|s| s.to_string()),
    };

    let status = match harvest_request::request_harvest(&state.pool, bwb_id, opts).await {
        Ok(HarvestRequestOutcome::Created(job)) => {
            tracing::info!(
                job_id = %job.id,
                bwb_id = %bwb_id,
                slug = slug.unwrap_or("-"),
                priority = EDITOR_HARVEST_PRIORITY,
                "created editor-requested harvest job"
            );
            "queued"
        }
        Ok(HarvestRequestOutcome::AlreadyQueued { .. }) => "already_queued",
        // The law is exhausted and must be reset via the admin UI first. The
        // frontend already treats `harvest_exhausted` as a terminal status.
        Ok(HarvestRequestOutcome::Exhausted) => "harvest_exhausted",
        // Unreachable: the editor never sends a date. Treated as an error so
        // it can't silently pass as "queued" if that ever changes.
        Ok(HarvestRequestOutcome::InvalidDate { reason }) => {
            tracing::error!(bwb_id = %bwb_id, reason = %reason, "unexpected invalid date on editor harvest path");
            "error"
        }
        Err(e) => {
            tracing::error!(error = %e, bwb_id = %bwb_id, "failed to create harvest job");
            "error"
        }
    };

    HarvestResponse {
        bwb_id: bwb_id.to_string(),
        status: status.to_string(),
        slug: slug.map(|s| s.to_string()),
    }
}
