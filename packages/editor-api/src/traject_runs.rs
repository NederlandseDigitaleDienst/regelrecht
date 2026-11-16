//! Runs API: the harvests, enrichments and conversions requested in one
//! traject, running or finished, for the "Verwerkingen" page of a law and the
//! details of a task. Every member of the traject sees every run, with who
//! requested it; the query lives in `regelrecht_pipeline::traject_runs`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use regelrecht_pipeline::traject_runs::{self, LawFilter, TrajectRun};

use crate::accounts::AccountRecord;
use crate::state::AppState;
use crate::trajects::{require_membership, resolve_traject_ref};

#[derive(Serialize)]
pub struct TrajectRunList {
    /// Newest first, at most `traject_runs::MAX_RUNS`.
    pub runs: Vec<TrajectRun>,
}

/// The optional filter of the runs list: the runs about one law.
#[derive(Deserialize)]
pub struct RunsQuery {
    /// The law's `$id`. Without it the list holds every run of the traject.
    law_id: Option<String>,
    /// The law's BWB id, to include the traject harvest that fetched it.
    /// Read only together with `law_id`.
    bwb_id: Option<String>,
}

impl RunsQuery {
    fn law_filter(&self) -> Option<LawFilter<'_>> {
        let law_id = self.law_id.as_deref().filter(|id| !id.is_empty())?;
        Some(LawFilter {
            law_id,
            bwb_id: self.bwb_id.as_deref().filter(|id| !id.is_empty()),
        })
    }
}

/// GET /api/trajects/{traject_ref}/runs?law_id=…&bwb_id=…
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(traject_ref): Path<String>,
    Query(query): Query<RunsQuery>,
) -> Result<Json<TrajectRunList>, (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;
    require_membership(pool, traject_id, account.id)
        .await
        .map_err(|status| (status, "not a member of this traject".to_string()))?;
    let runs = traject_runs::list_traject_runs(pool, traject_id, account.id, query.law_filter())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn query(law_id: Option<&str>, bwb_id: Option<&str>) -> RunsQuery {
        RunsQuery {
            law_id: law_id.map(str::to_owned),
            bwb_id: bwb_id.map(str::to_owned),
        }
    }

    #[test]
    fn filters_on_a_law_only_when_one_is_named() {
        assert!(query(None, None).law_filter().is_none());
        assert!(query(Some(""), Some("BWBR0018451")).law_filter().is_none());
        assert!(query(None, Some("BWBR0018451")).law_filter().is_none());
    }

    #[test]
    fn carries_the_bwb_id_along_with_the_law() {
        let q = query(Some("wet_op_de_zorgtoeslag"), Some("BWBR0018451"));
        let filter = q.law_filter().expect("a named law should give a filter");
        assert_eq!(filter.law_id, "wet_op_de_zorgtoeslag");
        assert_eq!(filter.bwb_id, Some("BWBR0018451"));
        let q = query(Some("wet_op_de_zorgtoeslag"), Some(""));
        assert_eq!(q.law_filter().map(|f| f.bwb_id), Some(None));
    }
}
