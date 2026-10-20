//! Het root-pad van de traject-eigen bron verzetten en meteen zien dat de
//! bibliotheek meebeweegt.
//!
//! Een traject dat op repo-root staat indexeert élke `map/map/*.yaml` als
//! wet — ook de werkmappen die naast het corpus op de branch staan. Die
//! verschijnen dan als wetten zonder artikelen. De remedie is het root-pad
//! op de corpusmap zetten; deze test legt vast dat één PATCH daarvoor
//! genoeg is: de gecachete `TrajectCorpus` wordt ongeldig gemaakt, de
//! volgende listing scant opnieuw onder het nieuwe pad, en wat erbuiten
//! ligt telt niet meer mee.
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam; de repo-namen zijn fictief (publieke repo).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{insert_traject, state_with_user_token_mode, traject_ref, verified_session};

use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::dto::PaginationParams;
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::list_traject_corpus_laws;
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};
use regelrecht_editor_api::trajects::{self, UpdateTrajectRequest};

use regelrecht_pipeline::test_utils::TestDb;

const OWN_REPO_OWNER: &str = "example-org";
const OWN_REPO_NAME: &str = "regelrecht-corpus-example";
const OWN_BRANCH: &str = "traject/voorbeeld";
const CORPUS_SUBPATH: &str = "regulation/nl";
const CORPUS_LAW: &str = "wet_alpha";
/// Mapnaam náást het corpus op dezelfde branch: op repo-root leest de
/// indexscan die als wet-id, precies het probleem dat het nieuwe root-pad
/// oplost.
const WORKDIR_LAW: &str = "generatoren";
const USER_TOKEN: &str = "user-token";
const EMAIL: &str = "alice@test.local";

async fn seed_owner(pool: &PgPool) -> (AccountRecord, String) {
    let sub = format!("sub-{EMAIL}");
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&sub)
    .bind(EMAIL)
    .bind("Test User")
    .fetch_one(pool)
    .await
    .unwrap();
    (
        AccountRecord {
            id,
            person_sub: sub.clone(),
            email: EMAIL.to_string(),
            name: "Test User".to_string(),
        },
        sub,
    )
}

/// Traject met één bron: de eigen GitHub-repo op repo-root (`gh_path`
/// NULL), zoals een traject dat bij aanmaken geen subpath meekreeg.
async fn seeded_traject(pool: &PgPool, owner_id: Uuid) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 $2, $3, $4, 'main', NULL,
                 0, 'example-unset-token-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(OWN_REPO_OWNER)
    .bind(OWN_REPO_NAME)
    .bind(OWN_BRANCH)
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

fn cookie_headers(oauth: &GithubOAuth, account_id: Uuid) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            oauth, account_id, USER_TOKEN,
        ))
        .unwrap(),
    );
    headers
}

/// De branch zoals hij op GitHub staat: het corpus in `regulation/nl`,
/// plus een werkmap ernaast die toevallig ook YAML bevat.
async fn mount_tree(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO_OWNER}/{OWN_REPO_NAME}/git/trees/{OWN_BRANCH}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {
                    "path": format!("{CORPUS_SUBPATH}/wet/{CORPUS_LAW}/2025-01-01.yaml"),
                    "type": "blob",
                    "sha": "blob-wet",
                },
                {
                    "path": format!("werkbank/{WORKDIR_LAW}/poorten.yaml"),
                    "type": "blob",
                    "sha": "blob-werkbank",
                },
            ],
        })))
        .mount(server)
        .await;
}

fn all_laws_query() -> Query<PaginationParams> {
    Query(PaginationParams {
        offset: 0,
        limit: None,
        q: None,
        ids: None,
        source: None,
    })
}

#[tokio::test]
async fn changing_the_root_path_reindexes_the_traject_corpus() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());
    mount_tree(&server).await;

    let db = TestDb::new().await;
    let oauth = GithubOAuth::for_tests(true);
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());
    let (alice, sub) = seed_owner(&db.pool).await;
    let headers = cookie_headers(&oauth, alice.id);
    let traject_id = seeded_traject(&db.pool, alice.id).await;
    let tref = traject_ref(traject_id);

    // Op repo-root telt de werkmap mee als wet — het symptoom.
    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&sub, EMAIL).await,
        Path(tref.clone()),
        all_laws_query(),
        headers.clone(),
    )
    .await
    .expect("listing op repo-root moet slagen");
    let mut ids: Vec<&str> = laws.0.iter().map(|l| l.law_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![WORKDIR_LAW, CORPUS_LAW],
        "op repo-root indexeert de scan ook wat naast het corpus staat"
    );

    // Eén PATCH van de eigenaar verzet het root-pad.
    let status = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: None,
            description: None,
            scope: None,
            status: None,
            repo_path: Some(CORPUS_SUBPATH.to_string()),
        }),
    )
    .await
    .expect("de eigenaar mag het root-pad verzetten");
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Zonder extra herlaad: de cache is ongeldig gemaakt, dus deze
    // listing scant opnieuw — nu onder `regulation/nl`.
    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&sub, EMAIL).await,
        Path(tref),
        all_laws_query(),
        headers,
    )
    .await
    .expect("listing onder het nieuwe root-pad moet slagen");
    let ids: Vec<&str> = laws.0.iter().map(|l| l.law_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![CORPUS_LAW],
        "alleen wat onder het nieuwe root-pad staat telt nog als wet"
    );
}
