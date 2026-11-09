//! De AI-keuze op de werkdocument-upload (`?llm=`), aan de handler-kant.
//!
//! Twee dingen worden hier vastgelegd. Ten eerste de weigering: een formaat dat
//! alleen met een taalmodel om te zetten is (`.doc`) wordt zónder toestemming
//! met 400 geweigerd — er ontstaat geen job die toch niets anders kan dan
//! falen, en er wordt niets opgeslagen. Ten tweede de doorlaat: een formaat met
//! deterministische converter (`.docx`) gaat zónder toestemming gewoon door
//! (202) en draagt `allow_llm: false` de pipeline in, die de belofte daar
//! afdwingt.
//!
//! Zelfde hermetische opzet als `traject_harvest_request_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{empty_state, local_traject, seed_account, session_for, traject_ref};

use axum::body::Body;
use axum::extract::{Extension, FromRequest, Multipart, Path, Query, State};
use axum::http::{Request, StatusCode};
use pretty_assertions::assert_eq;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{upload_traject_document, UploadDocumentQuery};
use regelrecht_editor_api::state::AppState;

use regelrecht_pipeline::test_utils::TestDb;

const BOUNDARY: &str = "X-REGELRECHT-TEST-BOUNDARY";

fn account(account_id: Uuid) -> AccountRecord {
    AccountRecord {
        id: account_id,
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

/// Bouw de `Multipart`-extractor met één `file`-veld, zoals de browser 'm
/// stuurt. Handmatig in elkaar gezet omdat de handler de extractor krijgt, niet
/// een router — dit test de handler, niet axum.
async fn multipart_with_file(filename: &str, content_type: &str, bytes: &[u8]) -> Multipart {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    let request = Request::builder()
        .method("POST")
        .uri("/api/trajects/x/corpus/documents/upload")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap();
    Multipart::from_request(request, &()).await.unwrap()
}

/// De query-parameter zoals axum 'm zou parsen: `Some("0")`/`Some("1")`, of
/// afwezig.
fn query(llm: Option<&str>) -> Query<UploadDocumentQuery> {
    Query(UploadDocumentQuery {
        llm: llm.map(str::to_string),
    })
}

async fn upload(
    state: AppState,
    sub: &str,
    account_id: Uuid,
    tref: &str,
    filename: &str,
    content_type: &str,
    llm: Option<&str>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (status, _body) = upload_traject_document(
        State(state),
        Extension(account(account_id)),
        session_for(sub).await,
        Path(tref.to_string()),
        query(llm),
        axum::http::HeaderMap::new(),
        multipart_with_file(filename, content_type, b"wat bytes").await,
    )
    .await?;
    Ok(status)
}

/// `.doc` heeft geen deterministische converter. Zonder toestemming is de enige
/// eerlijke uitkomst een weigering nu — geen job, geen opgeslagen bytes.
#[tokio::test]
async fn upload_without_llm_permission_rejects_llm_only_format() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let err = upload(
        state,
        &sub,
        owner,
        &tref,
        "brief.doc",
        "application/msword",
        Some("0"),
    )
    .await
    .expect_err("een .doc zonder AI-toestemming moet geweigerd worden");

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("alleen met AI"),
        "de weigering legt uit waarom, kreeg: {}",
        err.1
    );

    let (jobs, uploads): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM jobs), (SELECT COUNT(*) FROM document_uploads)",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(jobs, 0, "geen job die toch alleen kan falen");
    assert_eq!(uploads, 0, "de bytes zijn nergens opgeslagen");
}

/// `.docx` gaat via pandoc en heeft geen taalmodel nodig, dus zonder
/// toestemming is dit gewoon een normale conversie — met `allow_llm: false` in
/// de payload, zodat de pipeline niet alsnog kan uitwijken.
#[tokio::test]
async fn upload_without_llm_permission_accepts_deterministic_format() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let status = upload(
        state,
        &sub,
        owner,
        &tref,
        "rapport.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("0"),
    )
    .await
    .expect("een .docx kan zonder AI en hoort gewoon geaccepteerd te worden");
    assert_eq!(status, StatusCode::ACCEPTED);

    let (job_type, payload): (String, serde_json::Value) =
        sqlx::query_as("SELECT job_type::text, payload FROM jobs")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(job_type, "document_convert");
    assert_eq!(payload["allow_llm"], false);
    assert_eq!(payload["target_path"], "rapport.md");
}

/// Mét toestemming mag hetzelfde `.doc` wél door; de keuze reist als
/// `allow_llm: true` mee zodat de pipeline de agent mag starten.
#[tokio::test]
async fn upload_with_llm_permission_accepts_llm_only_format() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let status = upload(
        state,
        &sub,
        owner,
        &tref,
        "brief.doc",
        "application/msword",
        Some("1"),
    )
    .await
    .expect("met toestemming is een .doc gewoon toegestaan");
    assert_eq!(status, StatusCode::ACCEPTED);

    let (payload,): (serde_json::Value,) = sqlx::query_as("SELECT payload FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(payload["allow_llm"], true);
}

/// Een client die de parameter helemaal niet meestuurt (oude frontend, curl)
/// krijgt de veilige uitkomst: geen toestemming.
#[tokio::test]
async fn missing_llm_parameter_counts_as_no_permission() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let err = upload(
        state,
        &sub,
        owner,
        &tref,
        "brief.doc",
        "application/msword",
        None,
    )
    .await
    .expect_err("zonder parameter is er geen toestemming");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
