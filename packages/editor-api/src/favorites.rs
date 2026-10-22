//! Favorieten, per plek waar je werkt.
//!
//! `traject_id IS NULL` is de Corpus-juris-set: wat je sterrt terwijl je
//! bladert. Een traject heeft zijn eigen set, want een favoriet is "de wet waar
//! ik hier steeds naar terugga" en "hier" is het traject. De twee sets kruisen
//! elkaar niet: een ster in Corpus juris duikt niet op in een traject.
//!
//! De traject-varianten hangen onder `/api/trajects/{ref}/favorites` en checken
//! het lidmaatschap zelf, dezelfde re-check die de andere traject-routes doen.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_auth::SESSION_KEY_SUB;

use crate::accounts::AccountRecord;
use crate::state::AppState;
use crate::trajects::{require_membership, resolve_traject_ref};

async fn get_person_sub(session: &Session) -> Result<String, StatusCode> {
    session
        .get::<String>(SESSION_KEY_SUB)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn validate_law_id(law_id: &str) -> Result<(), StatusCode> {
    // .len() returns bytes, which equals character count for ASCII-only law IDs.
    if law_id.is_empty() || law_id.len() > 256 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Resolve the traject ref and re-check membership, so a favorite can never be
/// read from or written to a traject you are not in. Same guard the other
/// traject routes apply. The role within the traject does not matter here: who
/// may star is decided by the route's tier (read on editor-reader, write on
/// editor-writer). The Corpus juris set is different: any account may write
/// it, since it touches nothing but the account itself.
async fn scope_traject(
    state: &AppState,
    account: &AccountRecord,
    traject_ref: &str,
) -> Result<Uuid, StatusCode> {
    let pool = get_pool(state)?;
    let traject_id = resolve_traject_ref(pool, traject_ref)
        .await
        .map_err(|(status, _)| status)?;
    require_membership(pool, traject_id, account.id).await?;
    Ok(traject_id)
}

/// De drie queries in één, met `traject_id` als de enige variabele. `IS NOT
/// DISTINCT FROM` matcht NULL op NULL, zodat de Corpus-juris-set met dezelfde
/// query wordt geraakt als een traject-set.
async fn list_for(
    state: &AppState,
    session: &Session,
    traject_id: Option<Uuid>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let person_sub = get_person_sub(session).await?;
    let pool = get_pool(state)?;

    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT law_id FROM user_favorites \
         WHERE person_sub = $1 AND traject_id IS NOT DISTINCT FROM $2 \
         ORDER BY created_at LIMIT $3",
    )
    .bind(&person_sub)
    .bind(traject_id)
    .bind(MAX_FAVORITES)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to fetch favorites");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows.into_iter().map(|(id,)| id).collect()))
}

async fn add_for(
    state: &AppState,
    session: &Session,
    traject_id: Option<Uuid>,
    law_id: &str,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(law_id)?;
    let person_sub = get_person_sub(session).await?;
    let pool = get_pool(state)?;

    // ON CONFLICT zonder doel: welke van de twee partiële unieke indexen het
    // opvangt hangt af van `traject_id`, en dat hoeft de query niet te weten.
    // The WHERE caps a set at MAX_FAVORITES: every account may write its
    // Corpus juris set, so the table must not grow without bound per person.
    let result = sqlx::query(
        "INSERT INTO user_favorites (person_sub, law_id, traject_id) \
         SELECT $1, $2, $3 \
         WHERE (SELECT count(*) FROM user_favorites \
                WHERE person_sub = $1 AND traject_id IS NOT DISTINCT FROM $3) < $4 \
         ON CONFLICT DO NOTHING",
    )
    .bind(&person_sub)
    .bind(law_id)
    .bind(traject_id)
    .bind(MAX_FAVORITES)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to add favorite");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() > 0 {
        return Ok(StatusCode::CREATED);
    }
    // Nothing inserted: either the law was already a favorite (fine), or the
    // set is full (refused, so the client can roll its star back).
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM user_favorites \
         WHERE person_sub = $1 AND law_id = $2 AND traject_id IS NOT DISTINCT FROM $3)",
    )
    .bind(&person_sub)
    .bind(law_id)
    .bind(traject_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to check favorite");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if exists {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::CONFLICT)
    }
}

/// Most favorites one set (Corpus juris, or one traject) holds; the list
/// endpoint returns at most this many too.
const MAX_FAVORITES: i64 = 1000;

async fn remove_for(
    state: &AppState,
    session: &Session,
    traject_id: Option<Uuid>,
    law_id: &str,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(law_id)?;
    let person_sub = get_person_sub(session).await?;
    let pool = get_pool(state)?;

    sqlx::query(
        "DELETE FROM user_favorites \
         WHERE person_sub = $1 AND law_id = $2 AND traject_id IS NOT DISTINCT FROM $3",
    )
    .bind(&person_sub)
    .bind(law_id)
    .bind(traject_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to remove favorite");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/favorites — de Corpus-juris-set van de ingelogde gebruiker.
pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<String>>, StatusCode> {
    list_for(&state, &session, None).await
}

/// PUT /api/favorites/{law_id} — sterren in Corpus juris.
pub async fn add(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    add_for(&state, &session, None, &law_id).await
}

/// DELETE /api/favorites/{law_id} — ontsterren in Corpus juris.
pub async fn remove(
    State(state): State<AppState>,
    session: Session,
    Path(law_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    remove_for(&state, &session, None, &law_id).await
}

/// GET /api/trajects/{traject_ref}/favorites — de set van dit traject.
pub async fn list_traject(
    State(state): State<AppState>,
    session: Session,
    axum::Extension(account): axum::Extension<AccountRecord>,
    Path(traject_ref): Path<String>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let traject_id = scope_traject(&state, &account, &traject_ref).await?;
    list_for(&state, &session, Some(traject_id)).await
}

/// PUT /api/trajects/{traject_ref}/favorites/{law_id} — sterren in dit traject.
pub async fn add_traject(
    State(state): State<AppState>,
    session: Session,
    axum::Extension(account): axum::Extension<AccountRecord>,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let traject_id = scope_traject(&state, &account, &traject_ref).await?;
    add_for(&state, &session, Some(traject_id), &law_id).await
}

/// DELETE /api/trajects/{traject_ref}/favorites/{law_id} — ontsterren in dit traject.
pub async fn remove_traject(
    State(state): State<AppState>,
    session: Session,
    axum::Extension(account): axum::Extension<AccountRecord>,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let traject_id = scope_traject(&state, &account, &traject_ref).await?;
    remove_for(&state, &session, Some(traject_id), &law_id).await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};
    use tower_sessions_memory_store::MemoryStore;

    use super::*;
    use crate::config::AppConfig;
    use crate::state::CorpusState;
    use crate::traject_corpus::TrajectCorpusCache;
    use regelrecht_pipeline::test_utils::TestDb;

    fn state_with(pool: sqlx::PgPool) -> AppState {
        AppState {
            corpus: Arc::new(RwLock::new(CorpusState::empty())),
            oidc_client: None,
            end_session_url: None,
            config: Arc::new(AppConfig {
                oidc: None,
                base_url: None,
                github_oauth: None,
                task_enrich_provider: "claude".to_string(),
            }),
            http_client: regelrecht_auth::http_client(),
            pool: Some(pool),
            pipeline_api_url: None,
            harvest_admin_url: None,
            reload_lock: Arc::new(Mutex::new(())),
            integrity: Default::default(),
            trajects: Arc::new(TrajectCorpusCache::new()),
        }
    }

    async fn session_for(sub: &str) -> Session {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        session.insert(SESSION_KEY_SUB, sub).await.unwrap();
        session
    }

    #[tokio::test]
    async fn a_corpus_juris_set_is_capped_and_a_repeat_star_is_no_error() {
        let db = TestDb::new().await;
        let state = state_with(db.pool.clone());
        let session = session_for("sub-reader").await;

        // Fill the set to the cap directly; the handler is about the edge.
        sqlx::query(
            "INSERT INTO user_favorites (person_sub, law_id, traject_id) \
             SELECT 'sub-reader', 'wet_' || n, NULL FROM generate_series(1, $1) AS n",
        )
        .bind(MAX_FAVORITES as i32)
        .execute(&db.pool)
        .await
        .unwrap();

        // Starring a law that is already in the set is still fine...
        assert_eq!(
            add_for(&state, &session, None, "wet_1").await,
            Ok(StatusCode::NO_CONTENT)
        );
        // ...but one more law than the cap is refused.
        assert_eq!(
            add_for(&state, &session, None, "wet_extra").await,
            Err(StatusCode::CONFLICT)
        );
        // Another person's set is not affected by this one being full.
        let other = session_for("sub-other").await;
        assert_eq!(
            add_for(&state, &other, None, "wet_extra").await,
            Ok(StatusCode::CREATED)
        );
    }
}
