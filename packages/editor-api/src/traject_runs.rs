//! Runs API: the harvests, enrichments and conversions requested in one
//! traject, running or finished, for the "Verwerkingen" overview. Every member
//! of the traject sees every run, with who requested it; the query lives in
//! `regelrecht_pipeline::traject_runs`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Serialize;

use regelrecht_pipeline::traject_runs::{self, TrajectRun};

use crate::accounts::AccountRecord;
use crate::state::AppState;
use crate::trajects::{require_membership, resolve_traject_ref};

#[derive(Serialize)]
pub struct TrajectRunList {
    /// Newest first, at most `traject_runs::MAX_RUNS`.
    pub runs: Vec<TrajectRun>,
}

/// GET /api/trajects/{traject_ref}/runs
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(traject_ref): Path<String>,
) -> Result<Json<TrajectRunList>, (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;
    require_membership(pool, traject_id, account.id)
        .await
        .map_err(|status| (status, "not a member of this traject".to_string()))?;
    let runs = traject_runs::list_traject_runs(pool, traject_id, account.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, traject_ref = %traject_ref, "listing traject runs failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "listing runs failed".to_string(),
            )
        })?;
    Ok(Json(TrajectRunList { runs }))
}
