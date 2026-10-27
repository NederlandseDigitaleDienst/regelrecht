//! Deployed-achtige federatie-configuratie voor de "Wet toevoegen"-promote:
//! een read-only seed (het centrale corpus) plus een **GitHub-backed
//! writable-own met `gh_path`-subpath en ZONDER geconfigureerd service-token**
//! — precies de pr952-preview-config waarin promote met 403 "Source is
//! read-only" faalde terwijl de deployment in de user-token-schrijfmodus
//! staat (`github.user_oauth` / `GITHUB_USER_TOKEN_REQUIRED`): het traject is
//! aangemaakt op een user-gekozen repo, dus er bestaat bewust geen
//! `CORPUS_AUTH_*`-token voor die repo (fail-closed) en elke write hoort via
//! het gekoppelde GitHub-token van de acterende gebruiker te lopen.
//!
//! Zonder de fix (writability-gate die de user-token-modus kent + een
//! GitHubApiBackend die zonder rest-token buffert en bij `persist` met het
//! override-token commit én de traject-branch bootstrapt) faalt de
//! promote-scenario hieronder aantoonbaar met die 403.
//!
//! GitHub wordt gespeeld door wiremock; `GITHUB_API_BASE` (de test-seam in
//! `GithubClient::new`) wijst de clients daarheen. Alle scenario's staan in
//! ÉÉN test zodat die env-var niet tussen parallelle tests kan racen.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, insert_traject, seed_account, session_for, state_with_user_token_mode,
    traject_ref,
};

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{promote_corpus_law, save_law};
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_promotie";
const OWN_REPO: &str = "example-org/example-traject-repo";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";

/// Traject zoals de deployed preview: een GitHub writable-own op een
/// user-gekozen repo (subpath, auth_ref waarvoor géén
/// `CORPUS_AUTH_*_TOKEN`-env bestaat → geen rest-token, backend read-only
/// at rest) plus een lokale read-seed die het centrale corpus speelt.
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

/// Volledige wet-map in het centrale corpus: twee versies plus één
/// scenario-file — de drie bestanden die de promote moet kopiëren.
fn write_central_law(central_dir: &std::path::Path) {
    let law_dir = central_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(law_dir.join("scenarios")).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2025\nmachine_readable: MARKER_2025\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("2024-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2024\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("scenarios").join("basis.feature"),
        "Feature: basis\n",
    )
    .unwrap();
}

/// Mocks voor de user-token-schrijfflow op de traject-repo:
/// branch-bootstrap (bestaat nog niet → aanmaken vanaf `main`) en de
/// Contents-PUT's, allemaal geauthenticeerd met het user-token.
async fn mount_write_mocks(server: &MockServer) {
    // Promote leest eerst of de wet al in de traject-repo staat. Die read
    // 404't (hij staat er niet, de branch bestaat nog niet eens), en een
    // 404 telt pas als afwezigheid zodra de repo zelf leesbaar blijkt —
    // anders zou een repo die dit token niet mag zien als "leeg" lezen.
    // Eén lookup per repo+token, daarna onthouden.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "full_name": OWN_REPO,
        })))
        .mount(server)
        .await;
    // Branch-check van de traject-branch: bestaat nog niet bij de eerste
    // bootstrap (404, eenmalig), daarna wél — de save-scenario bouwt het
    // traject-corpus opnieuw op (promote invalideert de cache) en diens
    // persist-bootstrap moet de bestaande branch zien in plaats van hem
    // nogmaals aan te maken.
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO}/git/ref/heads/{OWN_BRANCH}"
        )))
        .respond_with(ResponseTemplate::new(404))
        .up_to_n_times(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO}/git/ref/heads/{OWN_BRANCH}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": format!("refs/heads/{OWN_BRANCH}"),
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
    // Base-ref voor de branch-create.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/git/ref/heads/main")))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/heads/main",
            "object": { "sha": "base-sha" },
        })))
        .mount(server)
        .await;
    // Branch aanmaken — met het user-token (er ís geen ander token).
    Mock::given(method("POST"))
        .and(path(format!("/repos/{OWN_REPO}/git/refs")))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "ref": format!("refs/heads/{OWN_BRANCH}"),
        })))
        .expect(1)
        .mount(server)
        .await;
    // Contents-PUT's: de drie promote-bestanden (en later de save) landen
    // onder de subpath, geauthenticeerd als de gebruiker.
    for file in [
        format!("wet/{LAW_ID}/2025-01-01.yaml"),
        format!("wet/{LAW_ID}/2024-01-01.yaml"),
        format!("wet/{LAW_ID}/scenarios/basis.feature"),
    ] {
        Mock::given(method("PUT"))
            .and(path(format!(
                "/repos/{OWN_REPO}/contents/{OWN_SUBPATH}/{file}"
            )))
            .and(header("authorization", "Bearer user-token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "content": { "sha": format!("sha-{file}") },
            })))
            .mount(server)
            .await;
    }
}

/// PUT-paden (Contents API) die wiremock ontving, in volgorde.
async fn received_put_paths(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "PUT")
        .map(|r| r.url.path().to_string())
        .collect()
}

/// Eén test voor alle scenario's: de `GITHUB_API_BASE`-override is
/// proces-breed, dus parallelle tests met elk een eigen mockserver zouden
/// elkaars fetchers verhangen.
#[tokio::test]
async fn user_token_mode_writes_promote_and_save_through_the_traject_repo() {
    let server = MockServer::start().await;
    // Test-seam (zie `GithubClient::new`): alle clients in dit proces —
    // ook die diep in `build_traject_corpus` — praten tegen wiremock.
    std::env::set_var("GITHUB_API_BASE", server.uri());

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());

    let oauth = GithubOAuth::for_tests(true); // user-token-schrijfmodus aan
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

    mount_write_mocks(&server).await;

    // --- Scenario 1: geen gekoppeld GitHub-account → 428 (koppel-flow),
    // niet de oude 403 "Source is read-only". Fail-closed: zonder
    // user-token wordt er niets geschreven.
    let err = promote_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect_err("zonder gekoppeld token moet de promote weigeren");
    assert_eq!(
        err.0,
        StatusCode::PRECONDITION_REQUIRED,
        "verwacht de koppel-flow (428), kreeg: {} {}",
        err.0,
        err.1
    );
    assert_eq!(
        received_put_paths(&server).await,
        Vec::<String>::new(),
        "zonder token mag er niets richting GitHub geschreven zijn"
    );

    // --- Scenario 2: gekoppeld GitHub-account → de promote kopieert de
    // volledige wet-map naar de traject-repo, onder de subpath, met het
    // user-token (pre-fix faalde dit met 403 "Source is read-only").
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            account.id,
            "user-token",
        ))
        .unwrap(),
    );

    let response = promote_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), LAW_ID.to_string())),
        headers.clone(),
    )
    .await
    .expect("promote met gekoppeld token moet slagen");
    assert_eq!(response.0.law_id, LAW_ID);
    assert_eq!(response.0.copied_files, 3, "2 versies + 1 scenario");

    let puts = received_put_paths(&server).await;
    assert_eq!(puts.len(), 3, "drie Contents-PUT's, got: {puts:?}");
    for file in [
        format!("wet/{LAW_ID}/2025-01-01.yaml"),
        format!("wet/{LAW_ID}/2024-01-01.yaml"),
        format!("wet/{LAW_ID}/scenarios/basis.feature"),
    ] {
        let expected = format!("/repos/{OWN_REPO}/contents/{OWN_SUBPATH}/{file}");
        assert!(
            puts.contains(&expected),
            "PUT naar {expected} ontbreekt, got: {puts:?}"
        );
    }

    // --- Scenario 3: een gewone law-save door dezelfde gebruiker volgt
    // exact hetzelfde schrijfpad (zelfde backend, zelfde subpath, zelfde
    // user-token-autorisatie) — promote en save zijn hetzelfde pad.
    let body = format!("$id: {LAW_ID}\nname: Versie 2025 (bewerkt)\n");
    save_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref.clone(), LAW_ID.to_string())),
        headers,
        body,
    )
    .await
    .expect("law-save met gekoppeld token moet slagen");

    let puts = received_put_paths(&server).await;
    assert_eq!(
        puts.len(),
        4,
        "de save is één extra Contents-PUT, got: {puts:?}"
    );
    assert_eq!(
        puts[3],
        format!("/repos/{OWN_REPO}/contents/{OWN_SUBPATH}/wet/{LAW_ID}/2025-01-01.yaml"),
        "de save landt op hetzelfde subpath-schrijfpad als de promote"
    );

    // De branch-bootstrap (POST /git/refs, `.expect(1)` op de mock) is
    // precies één keer gebeurd: bij de eerste geslaagde persist, met het
    // user-token. wiremock verifieert dat bij drop van de server.
    std::env::remove_var("GITHUB_API_BASE");
}
