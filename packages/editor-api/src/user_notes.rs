//! Persoonlijke notities: private per-user notes on a law.
//!
//! The primary client contract is the **unified** annotations endpoint
//! (`corpus_handlers`): `PUT/GET /api/trajects/{ref}/corpus/laws/{law_id}/annotations`.
//! There a note's `regelrecht:visibility` property decides storage —
//! `"personal"` routes to this module's Postgres store (keyed by
//! `accounts.id`, never git), anything else goes to the shared sidecar
//! YAML on the traject's git branch (RFC-018 Decision 1) — and the GET
//! merges the caller's personal notes, so marked, into the returned
//! document. The UI never needs to know where a note lives.
//!
//! This module additionally exposes item-level routes for the personal
//! store (`/api/user/notes/{law_id}`, GET/POST/PUT/DELETE), which the
//! sidecar side has no analogue for: editing or deleting an individual
//! personal note.
//!
//! Rows mirror the W3C Web Annotation model of RFC-005: each note
//! serializes as an `Annotation` with a `TextualBody` (markdown by
//! default) and a law-level target that optionally carries the same
//! selector shape (TextQuoteSelector) as public notes — so one and the
//! same note object can be saved to either side.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::accounts::AccountRecord;
use crate::state::AppState;

/// Motivations accepted for a personal note. Deliberately narrower than
/// the full W3C vocabulary: personal notes carry free-form context, not
/// linking/tagging semantics (those belong to shared traject notes).
const ALLOWED_MOTIVATIONS: &[&str] = &["commenting", "questioning"];

/// Body formats accepted for a personal note. Markdown is the default
/// authoring format; plain text is kept for parity with RFC-005 examples.
const ALLOWED_FORMATS: &[&str] = &["text/markdown", "text/plain"];

/// Upper bound on the note body in bytes. Generous for hand-written
/// context, small enough that the table cannot be used as blob storage.
const MAX_BODY_VALUE_BYTES: usize = 64 * 1024;

/// Upper bound on the serialized target selector in bytes. Real
/// TextQuoteSelectors (exact + prefix + suffix + position hint) are well
/// under 1 KiB; the cap only blocks blob abuse.
const MAX_SELECTOR_BYTES: usize = 8 * 1024;

/// Upper bound on notes per user per law. Far above real use, so an
/// account cannot grow unbounded rows (each up to 64 KiB). Also bound as
/// the `LIMIT` in `list`, so the list can never silently truncate.
const MAX_NOTES_PER_LAW: i64 = 200;

/// Request body for creating or updating a note. `format` and
/// `motivation` fall back to the markdown/commenting defaults so the
/// minimal client payload is just `{"value": "..."}`. `selector`
/// optionally anchors the note to law text (W3C TextQuoteSelector shape,
/// stored verbatim and echoed back under `target.selector`).
///
/// `selector` is a double `Option` to keep absent and explicit `null`
/// apart on PUT: absent = keep the stored anchoring, `null` = detach it.
#[derive(Debug, Deserialize)]
pub struct NoteRequest {
    pub value: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub motivation: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub selector: Option<Option<serde_json::Value>>,
}

/// Deserialize a field so that an absent key stays `None` (via the serde
/// `default`) while a present key — including an explicit JSON `null` —
/// becomes `Some(...)`.
fn some_if_present<'de, D>(de: D) -> Result<Option<Option<serde_json::Value>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<serde_json::Value>::deserialize(de).map(Some)
}

/// A personal note in W3C Web Annotation shape (RFC-005), so clients can
/// treat it like any other note. `id`, `created` and `modified` are
/// server-managed extras the sidecar format does not carry.
#[derive(Debug, Serialize)]
pub struct UserNote {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub note_type: &'static str,
    pub motivation: String,
    pub target: NoteTarget,
    pub body: NoteBody,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NoteTarget {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct NoteBody {
    #[serde(rename = "type")]
    pub body_type: &'static str,
    pub value: String,
    pub purpose: String,
    pub format: String,
}

type NoteRow = (
    Uuid,
    String,
    String,
    String,
    Option<serde_json::Value>,
    DateTime<Utc>,
    DateTime<Utc>,
);

/// Columns every read/write returns, in `NoteRow` order.
const RETURNING: &str = "id, motivation, body_value, body_format, selector, created_at, updated_at";

impl UserNote {
    /// Build the annotation view from a [`NoteRow`] for the given law.
    fn from_row(law_id: &str, row: NoteRow) -> Self {
        let (id, motivation, body_value, body_format, selector, created_at, updated_at) = row;
        UserNote {
            id,
            note_type: "Annotation",
            target: NoteTarget {
                source: format!("regelrecht://{law_id}"),
                selector,
            },
            body: NoteBody {
                body_type: "TextualBody",
                value: body_value,
                // W3C `purpose` mirrors the annotation-level motivation
                // for a single-body note (same convention as the editor's
                // NoteCreator).
                purpose: motivation.clone(),
                format: body_format,
            },
            motivation,
            created: created_at,
            modified: updated_at,
        }
    }
}

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn validate_law_id(law_id: &str) -> Result<(), StatusCode> {
    // Corpus law `$id`s are lowercase snake_case slugs; dot and hyphen are
    // allowed as slug variants. The allowlist keeps junk (spaces, control
    // chars, arbitrary Unicode) out of storage and out of the
    // `regelrecht://` URI echoed in every response. `.len()` is bytes,
    // which equals character count for this ASCII-only alphabet.
    let valid_slug = law_id
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'));
    if law_id.is_empty() || law_id.len() > 256 || !valid_slug {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validate a create/update request. `format`/`motivation`/`selector`
/// stay `None` when absent: `create` fills in the markdown/commenting
/// defaults (selector stays law-level), `update` keeps the stored values
/// (absent = keep, so a client that only sends `{"value": ...}` cannot
/// silently reset metadata or drop the anchoring). An explicit
/// `"selector": null` is `Some(None)`: valid, and clears the anchoring
/// on update.
/// Borrowing validation of a request, so callers with a batch can reject
/// the whole batch up front before anything is stored.
pub fn validate(req: &NoteRequest) -> Result<(), StatusCode> {
    if req.value.trim().is_empty() || req.value.len() > MAX_BODY_VALUE_BYTES {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(format) = &req.format
        && !ALLOWED_FORMATS.contains(&format.as_str())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(motivation) = &req.motivation
        && !ALLOWED_MOTIVATIONS.contains(&motivation.as_str())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(Some(selector)) = &req.selector {
        // Stored verbatim (it is the client's anchoring, resolved
        // client-side like sidecar notes), but it must at least be a
        // JSON object with a `type` — the invariant every W3C selector
        // shares — and stay within the size cap.
        let is_object_with_type = selector
            .as_object()
            .is_some_and(|o| o.get("type").is_some_and(|t| t.is_string()));
        if !is_object_with_type {
            return Err(StatusCode::BAD_REQUEST);
        }
        let serialized_len = selector.to_string().len();
        if serialized_len > MAX_SELECTOR_BYTES {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn validate_request(
    req: NoteRequest,
) -> Result<
    (
        String,
        Option<String>,
        Option<String>,
        Option<Option<serde_json::Value>>,
    ),
    StatusCode,
> {
    validate(&req)?;
    Ok((req.value, req.format, req.motivation, req.selector))
}

/// Fetch the account's notes for a law, oldest first. Store-level so the
/// item endpoint and the unified annotations GET share one query.
pub async fn fetch_notes(
    pool: &sqlx::PgPool,
    account_id: Uuid,
    law_id: &str,
) -> Result<Vec<UserNote>, StatusCode> {
    validate_law_id(law_id)?;

    let rows: Vec<NoteRow> = sqlx::query_as(&format!(
        "SELECT {RETURNING} \
         FROM user_notes WHERE account_id = $1 AND law_id = $2 \
         ORDER BY created_at ASC LIMIT $3",
    ))
    .bind(account_id)
    .bind(law_id)
    .bind(MAX_NOTES_PER_LAW)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to fetch user notes");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(rows
        .into_iter()
        .map(|row| UserNote::from_row(law_id, row))
        .collect())
}

/// The account's notes for a law as W3C annotation JSON values, each
/// marked `"regelrecht:visibility": "personal"` — the shape the unified
/// annotations GET merges into the sidecar document.
pub async fn personal_annotation_values(
    pool: &sqlx::PgPool,
    account_id: Uuid,
    law_id: &str,
) -> Result<Vec<serde_json::Value>, StatusCode> {
    let notes = fetch_notes(pool, account_id, law_id).await?;
    notes
        .into_iter()
        .map(|note| {
            let mut value = serde_json::to_value(&note).map_err(|e| {
                tracing::error!(error = %e, "failed to serialize personal note");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            if let Some(map) = value.as_object_mut() {
                map.insert(
                    "regelrecht:visibility".to_string(),
                    serde_json::Value::String("personal".to_string()),
                );
            }
            Ok(value)
        })
        .collect()
}

/// Map a W3C annotation (as submitted to the unified annotations save) to
/// a personal-note request. Personal notes hold a single `TextualBody`
/// with a commenting/questioning motivation; other shapes get a concrete
/// error so the client knows the note must go the public route instead.
pub fn annotation_to_request(
    law_id: &str,
    annotation: &serde_json::Value,
) -> Result<NoteRequest, String> {
    let obj = annotation
        .as_object()
        .ok_or("a note must be a JSON object")?;

    if let Some(kind) = obj.get("type")
        && kind.as_str() != Some("Annotation")
    {
        return Err("a note's type must be \"Annotation\"".to_string());
    }

    let body = obj.get("body").ok_or("a personal note needs a body")?;
    let body_obj = body
        .as_object()
        .ok_or("a personal note supports a single TextualBody (multiple bodies are public-only)")?;
    if body_obj.get("type").and_then(|t| t.as_str()) != Some("TextualBody") {
        return Err(
            "a personal note's body must be a TextualBody (linking notes are public-only)"
                .to_string(),
        );
    }
    let value = body_obj
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or("a personal note's body needs a string value")?
        .to_string();
    let format = body_obj
        .get("format")
        .and_then(|f| f.as_str())
        .map(str::to_string);

    let motivation = obj
        .get("motivation")
        .and_then(|m| m.as_str())
        .map(str::to_string);

    let mut selector = None;
    if let Some(target) = obj.get("target") {
        let target_obj = target
            .as_object()
            .ok_or("a note's target must be an object")?;
        if let Some(source) = target_obj.get("source").and_then(|s| s.as_str())
            && source != format!("regelrecht://{law_id}")
        {
            return Err("a note's target does not match the law it is being saved to".to_string());
        }
        selector = match target_obj.get("selector") {
            // A JSON-null or absent selector is simply an unanchored note.
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(Some(value.clone())),
        };
    }

    Ok(NoteRequest {
        value,
        format,
        motivation,
        selector,
    })
}

/// Insert a note for the account, enforcing the per-law cap atomically.
///
/// The cap check races under READ COMMITTED (two concurrent creates can
/// both see count = cap-1), so creates are serialized per (account, law)
/// with a transaction-scoped advisory lock. With `dedupe`, a note whose
/// (motivation, value, format, selector) already exists is skipped and
/// `Ok(None)` is returned — the unified annotations save uses this so a
/// retried request is idempotent, mirroring the sidecar's dedup.
/// `Err(CONFLICT)` = cap reached.
pub async fn insert_note(
    pool: &sqlx::PgPool,
    account_id: Uuid,
    law_id: &str,
    req: NoteRequest,
    dedupe: bool,
) -> Result<Option<UserNote>, StatusCode> {
    let mut notes = insert_notes(pool, account_id, law_id, vec![req], dedupe).await?;
    Ok(notes.pop().flatten())
}

/// Insert a batch of notes atomically: one transaction, one advisory
/// lock, all-or-nothing. A failure anywhere (validation, cap) rolls the
/// whole batch back, so a caller can never end up with a half-saved
/// batch. Returns one entry per input, `None` where dedup skipped.
pub async fn insert_notes(
    pool: &sqlx::PgPool,
    account_id: Uuid,
    law_id: &str,
    reqs: Vec<NoteRequest>,
    dedupe: bool,
) -> Result<Vec<Option<UserNote>>, StatusCode> {
    validate_law_id(law_id)?;
    // Validate the whole batch before touching the database, so a bad
    // note rejects the batch without any store work.
    for req in &reqs {
        validate(req)?;
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(error = %e, "failed to begin transaction for user note create");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2, 0))")
        .bind(account_id)
        .bind(law_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to take user note advisory lock");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut results = Vec::with_capacity(reqs.len());
    for req in reqs {
        let (value, format, motivation, selector) = validate_request(req)?;
        let format = format.unwrap_or_else(|| "text/markdown".to_string());
        let motivation = motivation.unwrap_or_else(|| "commenting".to_string());
        // On create, absent and explicit-null selector both mean "no anchoring".
        let selector = selector.flatten();

        if dedupe {
            let (exists,): (bool,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM user_notes \
                 WHERE account_id = $1 AND law_id = $2 AND motivation = $3 \
                 AND body_value = $4 AND body_format = $5 \
                 AND selector IS NOT DISTINCT FROM $6)",
            )
            .bind(account_id)
            .bind(law_id)
            .bind(&motivation)
            .bind(&value)
            .bind(&format)
            .bind(&selector)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to dedupe user note");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            if exists {
                results.push(None);
                continue;
            }
        }

        let row: Option<NoteRow> = sqlx::query_as(&format!(
            "INSERT INTO user_notes (account_id, law_id, motivation, body_value, body_format, selector) \
             SELECT $1, $2, $3, $4, $5, $6 \
             WHERE (SELECT COUNT(*) FROM user_notes WHERE account_id = $1 AND law_id = $2) < $7 \
             RETURNING {RETURNING}",
        ))
        .bind(account_id)
        .bind(law_id)
        .bind(&motivation)
        .bind(&value)
        .bind(&format)
        .bind(&selector)
        .bind(MAX_NOTES_PER_LAW)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create user note");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        match row {
            Some(row) => results.push(Some(UserNote::from_row(law_id, row))),
            // Cap reached: bail out; the dropped transaction rolls back
            // every insert already made in this batch.
            None => return Err(StatusCode::CONFLICT),
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(error = %e, "failed to commit user note create");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(results)
}

/// GET /api/user/notes/{law_id} — the authenticated user's notes for a law,
/// oldest first.
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<UserNote>>, StatusCode> {
    let pool = get_pool(&state)?;
    Ok(Json(fetch_notes(pool, account.id, &law_id).await?))
}

/// POST /api/user/notes/{law_id} — create a note, returns it as 201.
/// 409 when the per-law cap is reached.
pub async fn create(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(law_id): Path<String>,
    Json(req): Json<NoteRequest>,
) -> Result<(StatusCode, Json<UserNote>), StatusCode> {
    let pool = get_pool(&state)?;
    match insert_note(pool, account.id, &law_id, req, false).await? {
        Some(note) => Ok((StatusCode::CREATED, Json(note))),
        // Unreachable without dedupe, but keep a sane mapping.
        None => Err(StatusCode::CONFLICT),
    }
}

/// PUT /api/user/notes/{law_id}/{note_id} — update a note's body;
/// `format`/`motivation`/`selector` are only changed when present in the
/// request (absent = keep; an explicit `"selector": null` detaches the
/// anchoring). 404 for a note that does not exist or belongs to another
/// account, so foreign note ids are indistinguishable from absent ones.
pub async fn update(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((law_id, note_id)): Path<(String, Uuid)>,
    Json(req): Json<NoteRequest>,
) -> Result<Json<UserNote>, StatusCode> {
    validate_law_id(&law_id)?;
    let (value, format, motivation, selector) = validate_request(req)?;
    let pool = get_pool(&state)?;

    // `selector` cannot use COALESCE (NULL is a meaningful new value:
    // detach), so a separate presence flag drives the CASE.
    let selector_present = selector.is_some();
    let selector_value = selector.flatten();

    let row: Option<NoteRow> = sqlx::query_as(&format!(
        "UPDATE user_notes SET motivation = COALESCE($4, motivation), body_value = $5, \
         body_format = COALESCE($6, body_format), \
         selector = CASE WHEN $8 THEN $7 ELSE selector END \
         WHERE id = $1 AND account_id = $2 AND law_id = $3 \
         RETURNING {RETURNING}",
    ))
    .bind(note_id)
    .bind(account.id)
    .bind(&law_id)
    .bind(&motivation)
    .bind(&value)
    .bind(&format)
    .bind(&selector_value)
    .bind(selector_present)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to update user note");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        Some(row) => Ok(Json(UserNote::from_row(&law_id, row))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// DELETE /api/user/notes/{law_id}/{note_id} — remove a note. 404 for a
/// note that does not exist or belongs to another account.
pub async fn remove(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((law_id, note_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    validate_law_id(&law_id)?;
    let pool = get_pool(&state)?;

    let result =
        sqlx::query("DELETE FROM user_notes WHERE id = $1 AND account_id = $2 AND law_id = $3")
            .bind(note_id)
            .bind(account.id)
            .bind(&law_id)
            .execute(pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to delete user note");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    if result.rows_affected() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
