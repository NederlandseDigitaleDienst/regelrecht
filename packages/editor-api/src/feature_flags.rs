use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

/// Key of the GitHub user-OAuth flag (PR #887). Shared with the write path
/// (`credentials::write_requires_user_token`), which reads this flag to
/// decide whether traject writes must carry the acting user's own token.
pub const GITHUB_USER_OAUTH: &str = "github.user_oauth";

/// Every flag the editor knows, with its default. This is also the allow-list
/// for `PUT /api/feature-flags/{key}`: a key missing here is rejected with 400,
/// and the frontend then silently reverts the toggle. The frontend keeps its
/// own copy in `frontend/src/composables/useFeatureFlags.js` (it must render
/// before the API answers); `useFeatureFlags.drift.test.js` reads this block
/// and fails when the two drift apart.
static DEFAULTS: LazyLock<HashMap<String, bool>> = LazyLock::new(|| {
    HashMap::from([
        ("panel.article_text".into(), true),
        ("panel.scenario_form".into(), true),
        ("panel.yaml_editor".into(), true),
        ("panel.machine_readable".into(), true),
        ("panel.notes".into(), true),
        // GitHub user-OAuth (spike, PR #887). One switch, two effects: it
        // shows the "Koppel GitHub-account" affordance in the account menu
        // AND makes traject writes require the acting user's own GitHub token
        // (`credentials::write_requires_user_token`) — linking is never
        // offered-but-inert. Default off so the spike stays invisible until
        // opted in.
        (GITHUB_USER_OAUTH.into(), false),
        // RegelRijm in the editor's Machine pane (RFC-054). Default off while
        // the reading is a draft.
        ("panel.regelrijm".into(), false),
    ])
});

fn defaults() -> HashMap<String, bool> {
    DEFAULTS.clone()
}

/// Overlay stored rows on the defaults, skipping keys that are no longer
/// registered. The `feature_flags` table keeps rows for retired flags (the
/// 0012 seed's `panel.execution_trace`, and any toggle ever written for a flag
/// since removed); without this filter they would still reach the frontend as
/// flags that no code reads and no PUT accepts.
fn merge_rows(
    flags: &mut HashMap<String, bool>,
    rows: Vec<regelrecht_pipeline::models::FeatureFlag>,
) {
    for flag in rows {
        if DEFAULTS.contains_key(&flag.key) {
            flags.insert(flag.key, flag.enabled);
        }
    }
}

/// Effective value of a single flag: the stored row when present, else the
/// registered default. Errors propagate — the write path treats "can't read
/// the flag" as a failure, never as "assume the default".
pub async fn flag_enabled(
    pool: &sqlx::PgPool,
    key: &str,
) -> regelrecht_pipeline::error::Result<bool> {
    Ok(regelrecht_pipeline::feature_flags::get_flag(pool, key)
        .await?
        .map(|flag| flag.enabled)
        .unwrap_or_else(|| DEFAULTS.get(key).copied().unwrap_or(false)))
}

pub async fn list_feature_flags(State(state): State<AppState>) -> Json<HashMap<String, bool>> {
    let Some(pool) = &state.pool else {
        return Json(defaults());
    };

    match regelrecht_pipeline::feature_flags::list_flags(pool).await {
        Ok(rows) => {
            let mut flags = defaults();
            merge_rows(&mut flags, rows);
            Json(flags)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to fetch feature flags, using defaults");
            Json(defaults())
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateFlag {
    pub enabled: bool,
}

pub async fn update_feature_flag(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<UpdateFlag>,
) -> impl IntoResponse {
    if !DEFAULTS.contains_key(&key) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown flag key '{}'", key)})),
        )
            .into_response();
    }

    let Some(pool) = &state.pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "no database configured"})),
        )
            .into_response();
    };

    match regelrecht_pipeline::feature_flags::upsert_flag(pool, &key, body.enabled, None).await {
        Ok(_) => {
            // Return the full flag map after update
            match regelrecht_pipeline::feature_flags::list_flags(pool).await {
                Ok(rows) => {
                    let mut flags = defaults();
                    merge_rows(&mut flags, rows);
                    Json(flags).into_response()
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to list flags after update");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to update feature flag");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use regelrecht_pipeline::models::FeatureFlag;

    fn row(key: &str, enabled: bool) -> FeatureFlag {
        FeatureFlag {
            key: key.into(),
            enabled,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn stored_rows_override_registered_flags_only() {
        let mut flags = defaults();
        merge_rows(
            &mut flags,
            vec![
                row("panel.notes", false),
                row("panel.execution_trace", true),
            ],
        );
        assert_eq!(flags.get("panel.notes"), Some(&false));
        assert!(!flags.contains_key("panel.execution_trace"));
        assert_eq!(flags.len(), DEFAULTS.len());
    }
}
