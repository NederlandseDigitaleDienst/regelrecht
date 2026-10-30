//! Request-gebonden reads op de writable-own source met het persoonlijke
//! GitHub-token van de ingelogde gebruiker (ticket: reads gebruiken het
//! user-token wanneer het server-token ontbreekt — het leespad-vervolg op
//! de user-token-schrijfflow van #952/#953).
//!
//! Deployed-achtige federatie-config, zoals `promote_user_token_write_test`:
//! een GitHub-backed **writable-own zonder geconfigureerd service-token**
//! (privé user-gekozen repo, fail-closed) plus een lokale seed als centraal
//! corpus, met de user-token-modus aan. Vóór de fix degradeerden alle reads
//! op die source stil (lege documentenlijst, 404 op documenten, seed-versies
//! van traject-bestanden); nu:
//!
//! * **mét** gekoppeld GitHub-account leest elke request-gebonden read de
//!   traject-repo met het user-token (documenten, scenario's, wet-bodies);
//! * **zónder** gekoppeld account falen precies die reads luid met 428 (de
//!   koppel-flow), terwijl reads die de writable-own niet raken (seed-wetten,
//!   de wettenlijst) gewoon blijven werken.
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam — daarom staan alle scenario's in ÉÉN test
//! (zelfde afweging als `promote_user_token_write_test.rs`). De indexscan
//! (Trees API) is hier bewust wél leesbaar zonder token: het ticket
//! repareert de request-gebonden reads, niet de scan — de fixture zet dus
//! een gevulde index neer en laat alleen de Contents-reads authenticatie
//! eisen.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, contents_file_json, insert_traject, seed_account, session_for,
    state_with_user_token_mode, traject_ref,
};

use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::dto::PaginationParams;
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{
    get_traject_corpus_law, get_traject_document, get_traject_scenario, list_traject_corpus_laws,
    list_traject_documents,
};
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};

use regelrecht_pipeline::test_utils::TestDb;

const SEED_LAW_ID: &str = "wet_voorbeeld_seed";
const OWN_LAW_ID: &str = "wet_voorbeeld_traject";
const OWN_REPO: &str = "example-org/example-traject-repo";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";
const USER_TOKEN: &str = "user-token";

const OWN_LAW_BODY: &str = "$id: wet_voorbeeld_traject\nname: TRAJECT_REPO_VERSIE\n";
const BRANCH_SCENARIO: &str = "Feature: traject-versie\n";
const DOCUMENT_BODY: &str = "# Notitie\n";

/// Traject zoals deployed: GitHub writable-own op een user-gekozen repo
/// (auth_ref zonder geconfigureerd `CORPUS_AUTH_*_TOKEN`-env → server-side
/// géén token) plus een lokale read-seed die het centrale corpus speelt.
async fn seeded_traject(pool: &PgPool, owner_id: Uuid, seed_dir: &std::path::Path) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 'example-org', 'example-traject-repo', $2, 'main', $3,
                 0, 'example-unset-token-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

/// Seed-wet met een scenario in het "centrale corpus".
fn write_seed_law(central_dir: &std::path::Path) {
    let law_dir = central_dir.join("wet").join(SEED_LAW_ID);
    std::fs::create_dir_all(law_dir.join("scenarios")).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {SEED_LAW_ID}\nname: SEED_VERSIE\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("scenarios").join("basis.feature"),
        "Feature: seed-versie\n",
    )
    .unwrap();
}

/// Trees-listing van de traject-repo: de indexscan ziet de traject-eigen
/// wet én het branch-scenario van de seed-wet. Bewust zonder auth-eis —
/// de fixture isoleert het request-gebonden Contents-leespad.
async fn mount_tree(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/git/trees/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {
                    "path": format!("{OWN_SUBPATH}/wet/{OWN_LAW_ID}/2025-01-01.yaml"),
                    "type": "blob",
                    "sha": "blob-own-law",
                },
            ],
        })))
        .mount(server)
        .await;
}

/// Contents-mocks die **alleen** met het user-token antwoorden; zonder
/// `Authorization: Bearer user-token` valt elke request door naar
/// wiremock's default 404 — precies wat GitHub op een privé-repo doet.
async fn mount_authenticated_contents(server: &MockServer, tref: &str) {
    // Wet-body van de traject-eigen wet.
    let own_law_path = format!("{OWN_SUBPATH}/wet/{OWN_LAW_ID}/2025-01-01.yaml");
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/contents/{own_law_path}")))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(contents_file_json(
            "2025-01-01.yaml",
            &own_law_path,
            OWN_LAW_BODY,
        )))
        .mount(server)
        .await;

    // Branch-kopie van het scenario van de seed-wet (write-target-routing:
    // de branchversie hoort de seed-kopie te overrulen).
    let scenario_path = format!("{OWN_SUBPATH}/wet/{SEED_LAW_ID}/scenarios/basis.feature");
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/contents/{scenario_path}")))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(contents_file_json(
            "basis.feature",
            &scenario_path,
            BRANCH_SCENARIO,
        )))
        .mount(server)
        .await;

    // Documentenmap + één werkdocument.
    let docs_dir = format!("{OWN_SUBPATH}/documents/{tref}");
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/contents/{docs_dir}")))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "name": "notitie.md",
                "path": format!("{docs_dir}/notitie.md"),
                "sha": "sha-notitie",
                "type": "file",
            },
        ])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO}/contents/{docs_dir}/notitie.md"
        )))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(contents_file_json(
            "notitie.md",
            &format!("{docs_dir}/notitie.md"),
            DOCUMENT_BODY,
        )))
        .mount(server)
        .await;
}

fn all_query() -> Query<PaginationParams> {
    Query(PaginationParams {
        offset: 0,
        limit: None,
        q: None,
        ids: None,
        source: None,
        enriched: false,
        loaded: false,
    })
}

/// Eén test voor alle scenario's: de `GITHUB_API_BASE`-override is
/// proces-breed, dus parallelle tests met elk een eigen mockserver zouden
/// elkaars fetchers verhangen.
#[tokio::test]
async fn user_token_reads_serve_the_private_traject_repo_and_fail_loud_without_link() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_seed_law(central.path());

    let oauth = GithubOAuth::for_tests(true); // user-token-modus aan
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());

    let (owner_id, sub) = seed_account(&db.pool, "alice@test.local").await;
    let account = AccountRecord {
        id: owner_id,
        person_sub: sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };
    let traject_id = seeded_traject(&db.pool, owner_id, central.path()).await;
    let tref = traject_ref(traject_id);

    mount_tree(&server).await;
    mount_authenticated_contents(&server, &tref).await;

    let mut linked_headers = HeaderMap::new();
    linked_headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth, account.id, USER_TOKEN,
        ))
        .unwrap(),
    );

    // ------------------------------------------------------------------
    // Scenario 1: GEEN gekoppeld GitHub-account. Elke read die de
    // writable-own source raakt faalt luid met 428 (de koppel-flow) —
    // niet met een stille lege lijst of een generieke 404. Eerst, zodat
    // geen enkele cache al met het token gevuld is.
    // ------------------------------------------------------------------
    let err = list_traject_documents(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref.clone()),
        HeaderMap::new(),
    )
    .await
    .expect_err("documentenlijst zonder koppeling moet weigeren, niet leeg teruggeven");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED, "{}", err.1);

    let err = get_traject_document(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), "notitie.md".to_string())),
        HeaderMap::new(),
    )
    .await
    .expect_err("document-GET zonder koppeling moet weigeren, niet 404'en");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED, "{}", err.1);

    let err = get_traject_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), OWN_LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect_err("wet-body uit de traject-repo zonder koppeling moet weigeren");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED, "{}", err.1);

    let err = get_traject_scenario(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((
            tref.clone(),
            SEED_LAW_ID.to_string(),
            "basis.feature".to_string(),
        )),
        HeaderMap::new(),
    )
    .await
    .expect_err("scenario-GET (write-target = traject-repo) zonder koppeling moet weigeren");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED, "{}", err.1);

    // …maar reads die de writable-own NIET raken blijven werken: de
    // seed-wet leest gewoon, en de wettenlijst (index-only) toont de
    // traject-eigen wet.
    let (status, _headers, body) = get_traject_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), SEED_LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect("seed-wet moet zonder koppeling leesbaar blijven");
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("SEED_VERSIE"), "got: {body}");

    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref.clone()),
        all_query(),
        HeaderMap::new(),
    )
    .await
    .expect("wettenlijst moet zonder koppeling blijven werken");
    assert!(
        laws.0.iter().any(|l| l.law_id == OWN_LAW_ID),
        "traject-eigen wet moet in de index staan, got: {:?}",
        laws.0.iter().map(|l| &l.law_id).collect::<Vec<_>>()
    );

    // ------------------------------------------------------------------
    // Scenario 2: MET gekoppeld GitHub-account lezen alle drie de
    // pad-groepen de traject-repo met het user-token.
    // ------------------------------------------------------------------
    let docs = list_traject_documents(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref.clone()),
        linked_headers.clone(),
    )
    .await
    .expect("documentenlijst met koppeling moet slagen");
    let paths: Vec<&str> = docs.0.documents.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(paths, vec!["notitie.md"]);

    let response = get_traject_document(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), "notitie.md".to_string())),
        linked_headers.clone(),
    )
    .await
    .expect("document-GET met koppeling moet slagen");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(bytes.to_vec()).unwrap(), DOCUMENT_BODY);

    let (status, _headers, body) = get_traject_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), OWN_LAW_ID.to_string())),
        linked_headers.clone(),
    )
    .await
    .expect("wet-body uit de traject-repo met koppeling moet slagen");
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("TRAJECT_REPO_VERSIE"),
        "de body moet uit de traject-repo komen, got: {body}"
    );

    let (status, _headers, body) = get_traject_scenario(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((
            tref.clone(),
            SEED_LAW_ID.to_string(),
            "basis.feature".to_string(),
        )),
        linked_headers.clone(),
    )
    .await
    .expect("scenario-GET met koppeling moet slagen");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body, BRANCH_SCENARIO,
        "de branchversie (via user-token) moet de seed-kopie overrulen"
    );

    std::env::remove_var("GITHUB_API_BASE");
}
