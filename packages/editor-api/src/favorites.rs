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
         ORDER BY created_at LIMIT 1000",
    )
    .bind(&person_sub)
    .bind(traject_id)
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
    let result = sqlx::query(
        "INSERT INTO user_favorites (person_sub, law_id, traject_id) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(&person_sub)
    .bind(law_id)
    .bind(traject_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to add favorite");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() > 0 {
        Ok(StatusCode::CREATED)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

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
