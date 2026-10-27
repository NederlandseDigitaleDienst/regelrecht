//! Integration tests for the persoonlijke-notities HTTP handlers.
//!
//! Same setup as `trajects_test.rs`: an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb` (exercising
//! `0024_user_notes.sql` end-to-end) and direct handler invocation with
//! inline axum extractors, skipping router/middleware plumbing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{empty_state, seed_account_record};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use pretty_assertions::assert_eq;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::state::AppState;
use regelrecht_editor_api::user_notes::{self, NoteRequest, UserNote};

use regelrecht_pipeline::test_utils::TestDb;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn note_req(value: &str) -> NoteRequest {
    NoteRequest {
        value: value.to_string(),
        format: None,
        motivation: None,
        selector: None,
    }
}

async fn create_note(
    state: &AppState,
    account: &AccountRecord,
    law_id: &str,
    value: &str,
) -> UserNote {
    let (status, Json(note)) = user_notes::create(
        State(state.clone()),
        Extension(account.clone()),
        Path(law_id.to_string()),
        Json(note_req(value)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    note
}

async fn list_notes(state: &AppState, account: &AccountRecord, law_id: &str) -> Vec<UserNote> {
    let Json(notes) = user_notes::list(
        State(state.clone()),
        Extension(account.clone()),
        Path(law_id.to_string()),
    )
    .await
    .unwrap();
    notes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn note_request_distinguishes_absent_and_null_selector() {
    // Absent key -> None (keep on update); explicit null -> Some(None)
    // (detach on update). This is the wire contract the double Option
    // in NoteRequest exists for.
    let absent: NoteRequest = serde_json::from_str(r#"{"value":"x"}"#).unwrap();
    assert_eq!(absent.selector, None);

    let null: NoteRequest = serde_json::from_str(r#"{"value":"x","selector":null}"#).unwrap();
    assert_eq!(null.selector, Some(None));

    let set: NoteRequest =
        serde_json::from_str(r#"{"value":"x","selector":{"type":"TextQuoteSelector"}}"#).unwrap();
    assert_eq!(
        set.selector,
        Some(Some(serde_json::json!({"type": "TextQuoteSelector"})))
    );
}

#[tokio::test]
async fn create_returns_w3c_annotation_shape_with_markdown_default() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "**belangrijk**: zie artikel 2").await;

    assert_eq!(note.note_type, "Annotation");
    assert_eq!(note.motivation, "commenting");
    assert_eq!(note.target.source, "regelrecht://test_wet");
    assert_eq!(note.body.body_type, "TextualBody");
    assert_eq!(note.body.value, "**belangrijk**: zie artikel 2");
    assert_eq!(note.body.format, "text/markdown");
    assert_eq!(note.body.purpose, "commenting");
}

#[tokio::test]
async fn list_returns_own_notes_oldest_first() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    create_note(&state, &alice, "test_wet", "eerste").await;
    create_note(&state, &alice, "test_wet", "tweede").await;
    create_note(&state, &alice, "andere_wet", "elders").await;

    let notes = list_notes(&state, &alice, "test_wet").await;
    let values: Vec<&str> = notes.iter().map(|n| n.body.value.as_str()).collect();
    assert_eq!(values, vec!["eerste", "tweede"]);
}

#[tokio::test]
async fn notes_are_private_per_account() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@example.org", "Bob").await;

    let note = create_note(&state, &alice, "test_wet", "alleen van alice").await;

    // Bob sees nothing.
    assert!(list_notes(&state, &bob, "test_wet").await.is_empty());

    // Bob cannot update Alice's note — indistinguishable from absent.
    let err = user_notes::update(
        State(state.clone()),
        Extension(bob.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("gekaapt")),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);

    // Bob cannot delete Alice's note either.
    let err = user_notes::remove(
        State(state.clone()),
        Extension(bob),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);

    // Alice's note is untouched.
    let notes = list_notes(&state, &alice, "test_wet").await;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body.value, "alleen van alice");
}

#[tokio::test]
async fn update_changes_body_and_bumps_modified() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "oud").await;

    let Json(updated) = user_notes::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(NoteRequest {
            value: "nieuw".to_string(),
            format: Some("text/plain".to_string()),
            motivation: Some("questioning".to_string()),
            selector: None,
        }),
    )
    .await
    .unwrap();

    assert_eq!(updated.id, note.id);
    assert_eq!(updated.body.value, "nieuw");
    assert_eq!(updated.body.format, "text/plain");
    assert_eq!(updated.motivation, "questioning");
    assert_eq!(updated.body.purpose, "questioning");
    assert_eq!(updated.created, note.created);
    assert!(updated.modified >= note.modified);

    // Absent format/motivation keep the stored values (a `{"value": ...}`
    // client cannot silently reset metadata to the defaults).
    let Json(again) = user_notes::update(
        State(state.clone()),
        Extension(alice),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("nog nieuwer")),
    )
    .await
    .unwrap();
    assert_eq!(again.body.value, "nog nieuwer");
    assert_eq!(again.body.format, "text/plain");
    assert_eq!(again.motivation, "questioning");
}

#[tokio::test]
async fn selector_roundtrips_and_is_kept_on_value_only_update() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    let selector = serde_json::json!({
        "type": "TextQuoteSelector",
        "exact": "zorgtoeslag",
        "prefix": "aanspraak op een ",
        "suffix": " ter grootte van"
    });
    let (status, Json(note)) = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "verankerde notitie".to_string(),
            format: None,
            motivation: None,
            selector: Some(Some(selector.clone())),
        }),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(note.target.selector, Some(selector.clone()));

    // A value-only PUT keeps the anchoring.
    let Json(updated) = user_notes::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(note_req("aangepaste tekst")),
    )
    .await
    .unwrap();
    assert_eq!(updated.target.selector, Some(selector.clone()));

    // And the selector survives a list read.
    let notes = list_notes(&state, &alice, "test_wet").await;
    assert_eq!(notes[0].target.selector, Some(selector.clone()));

    // An explicit `"selector": null` detaches the anchoring. Serde maps a
    // present-but-null field to `Some(None)` via the double Option.
    let Json(detached) = user_notes::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
        Json(NoteRequest {
            value: "los van de tekst".to_string(),
            format: None,
            motivation: None,
            selector: Some(None),
        }),
    )
    .await
    .unwrap();
    assert_eq!(detached.target.selector, None);

    // Invalid selectors are rejected: not an object / missing type.
    for bad in [
        serde_json::json!("tekst"),
        serde_json::json!({"exact": "x"}),
    ] {
        let err = user_notes::create(
            State(state.clone()),
            Extension(alice.clone()),
            Path("test_wet".to_string()),
            Json(NoteRequest {
                value: "x".to_string(),
                format: None,
                motivation: None,
                selector: Some(Some(bad)),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    // Oversized selector (> 8 KiB serialized) is rejected.
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: None,
            motivation: None,
            selector: Some(Some(serde_json::json!({
                "type": "TextQuoteSelector",
                "exact": "a".repeat(8 * 1024 + 1)
            }))),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_rejects_beyond_per_law_cap() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    // Fill up to the cap (200) directly, then the next create must 409.
    sqlx::query(
        "INSERT INTO user_notes (account_id, law_id, body_value) \
         SELECT $1, 'test_wet', 'notitie ' || n FROM generate_series(1, 200) n",
    )
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(note_req("te veel")),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);

    // Another law is unaffected by the cap.
    create_note(&state, &alice, "andere_wet", "past nog").await;
}

#[tokio::test]
async fn delete_removes_note() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    let note = create_note(&state, &alice, "test_wet", "weg ermee").await;

    let status = user_notes::remove(
        State(state.clone()),
        Extension(alice.clone()),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(list_notes(&state, &alice, "test_wet").await.is_empty());

    // Deleting again is a 404.
    let err = user_notes::remove(
        State(state.clone()),
        Extension(alice),
        Path(("test_wet".to_string(), note.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_invalid_input() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    // Empty / whitespace-only body.
    for value in ["", "   \n\t"] {
        let err = user_notes::create(
            State(state.clone()),
            Extension(alice.clone()),
            Path("test_wet".to_string()),
            Json(note_req(value)),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    // Unknown format / motivation.
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: Some("text/html".to_string()),
            motivation: None,
            selector: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(NoteRequest {
            value: "x".to_string(),
            format: None,
            motivation: Some("linking".to_string()),
            selector: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    // Oversized body value (> 64 KiB).
    let err = user_notes::create(
        State(state.clone()),
        Extension(alice.clone()),
        Path("test_wet".to_string()),
        Json(note_req(&"a".repeat(64 * 1024 + 1))),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);

    // Empty, oversized, and non-slug law ids (spaces, uppercase, slashes)
    // are rejected by every handler that takes one.
    for law_id in [
        String::new(),
        "x".repeat(257),
        "wet met spatie".to_string(),
        "Wet_Hoofdletter".to_string(),
        "wet/../elders".to_string(),
    ] {
        let err = user_notes::list(
            State(state.clone()),
            Extension(alice.clone()),
            Path(law_id.clone()),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);

        let err = user_notes::create(
            State(state.clone()),
            Extension(alice.clone()),
            Path(law_id.clone()),
            Json(note_req("x")),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);

        let err = user_notes::remove(
            State(state.clone()),
            Extension(alice.clone()),
            Path((law_id, Uuid::new_v4())),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);
    }
}

#[test]
fn annotation_to_request_maps_w3c_shape() {
    let annotation = serde_json::json!({
        "type": "Annotation",
        "motivation": "questioning",
        "target": {
            "source": "regelrecht://test_wet",
            "selector": {"type": "TextQuoteSelector", "exact": "zorgtoeslag"}
        },
        "body": {
            "type": "TextualBody",
            "value": "**waarom** geldt dit?",
            "format": "text/markdown",
            "purpose": "questioning"
        },
        "creator": "genegeerd — server kent de auteur"
    });

    let req = user_notes::annotation_to_request("test_wet", &annotation).unwrap();
    assert_eq!(req.value, "**waarom** geldt dit?");
    assert_eq!(req.format.as_deref(), Some("text/markdown"));
    assert_eq!(req.motivation.as_deref(), Some("questioning"));
    assert_eq!(
        req.selector,
        Some(Some(serde_json::json!({
            "type": "TextQuoteSelector",
            "exact": "zorgtoeslag"
        })))
    );
}

#[test]
fn annotation_to_request_rejects_unsupported_shapes() {
    // Wrong law target.
    let wrong_law = serde_json::json!({
        "target": {"source": "regelrecht://andere_wet"},
        "body": {"type": "TextualBody", "value": "x"}
    });
    assert!(user_notes::annotation_to_request("test_wet", &wrong_law).is_err());

    // Linking body (SpecificResource) is public-only.
    let linking = serde_json::json!({
        "body": {"type": "SpecificResource", "source": "regelrecht://test_wet/x"}
    });
    assert!(user_notes::annotation_to_request("test_wet", &linking).is_err());

    // Array bodies are public-only.
    let multi = serde_json::json!({
        "body": [{"type": "TextualBody", "value": "x"}]
    });
    assert!(user_notes::annotation_to_request("test_wet", &multi).is_err());
}

#[tokio::test]
async fn insert_note_dedupes_identical_notes_for_unified_save() {
    let db = TestDb::new().await;
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    let first = user_notes::insert_note(&db.pool, alice.id, "test_wet", note_req("zelfde"), true)
        .await
        .unwrap();
    assert!(first.is_some());

    // A retried unified save re-submits the same note: skipped, no dup row.
    let second = user_notes::insert_note(&db.pool, alice.id, "test_wet", note_req("zelfde"), true)
        .await
        .unwrap();
    assert!(second.is_none());

    // Without dedupe (item POST) the same content is a new note.
    let third = user_notes::insert_note(&db.pool, alice.id, "test_wet", note_req("zelfde"), false)
        .await
        .unwrap();
    assert!(third.is_some());

    let state = empty_state(db.pool.clone());
    assert_eq!(list_notes(&state, &alice, "test_wet").await.len(), 2);
}

#[tokio::test]
async fn personal_annotation_values_carry_the_visibility_marker() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;
    create_note(&state, &alice, "test_wet", "prive context").await;

    let values = user_notes::personal_annotation_values(&db.pool, alice.id, "test_wet")
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    let note = &values[0];
    assert_eq!(
        note.get("regelrecht:visibility").and_then(|v| v.as_str()),
        Some("personal")
    );
    assert_eq!(
        note.get("type").and_then(|v| v.as_str()),
        Some("Annotation")
    );
    assert!(note.get("id").is_some());
    assert_eq!(
        note.pointer("/body/value").and_then(|v| v.as_str()),
        Some("prive context")
    );
}

#[tokio::test]
async fn insert_notes_batch_is_all_or_nothing() {
    let db = TestDb::new().await;
    let alice = seed_account_record(&db.pool, "alice@example.org", "Alice").await;

    // A batch with an invalid note stores nothing at all.
    let err = user_notes::insert_notes(
        &db.pool,
        alice.id,
        "test_wet",
        vec![note_req("geldig"), note_req("   ")],
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::BAD_REQUEST);
    assert!(user_notes::fetch_notes(&db.pool, alice.id, "test_wet")
        .await
        .unwrap()
        .is_empty());

    // A batch that crosses the cap mid-way rolls back entirely.
    sqlx::query(
        "INSERT INTO user_notes (account_id, law_id, body_value) \
         SELECT $1, 'test_wet', 'notitie ' || n FROM generate_series(1, 199) n",
    )
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();
    let err = user_notes::insert_notes(
        &db.pool,
        alice.id,
        "test_wet",
        vec![note_req("past nog"), note_req("past niet meer")],
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
    assert_eq!(
        user_notes::fetch_notes(&db.pool, alice.id, "test_wet")
            .await
            .unwrap()
            .len(),
        199
    );
}
