//! Integratietests voor ticket "traject-indexscan: user-token fallback +
//! luid falen zonder token" (vervolg op #953/#955).
//!
//! Zelfde deployed-achtige federatie-config als `traject_own_index_test.rs`
//! — GitHub-backed writable-own zonder server-side `CORPUS_AUTH_*`-token,
//! lokale seed als centraal corpus, user-token-schrijfmodus aan — maar nu
//! rond de **indexscan** zelf:
//!
//! 1. **User-token scan (happy path).** De privé traject-repo is alleen
//!    leesbaar met het gekoppelde user-token (de Trees-mock matcht exact op
//!    `Bearer user-token`). De traject-bibliotheek toont de wet dan uit de
//!    traject-repo (priority 0), federatie met de seed blijft werken, en
//!    `/sources` is gezond.
//! 2. **Snapshot deelbaar, token niet.** Een tweede lid zónder gekoppeld
//!    GitHub-account wordt uit de gecachte snapshot bediend (metadata is
//!    voor alle leden zichtbaar) — maar na een cache-invalidatie kan de
//!    server de repo NIET meer scannen: het token is nergens bewaard, de
//!    herbouwde index faalt en de bibliotheek faalt **luid** met de
//!    428-koppel-flow in plaats van stil alleen seed-wetten te tonen.
//! 3. **Zelfheling.** Zodra een request mét gekoppeld token binnenkomt,
//!    wordt de kapotte (token-loos gescande) snapshot synchroon herbouwd
//!    en is de bibliotheek weer gezond.
//! 4. **Gekoppeld maar geen toegang.** Faalt de scan ook mét user-token,
//!    dan faalt de bibliotheek luid en geclassificeerd — hier: GitHub kent
//!    de repo niet op dat adres, dus 404 met een melding die het adres
//!    noemt. Geen 428 (opnieuw koppelen lost dit niet op) en geen stille
//!    fallback; `index_error` blijft zichtbaar op `/sources`.
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam — daarom staan alle scenario's in ÉÉN test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, insert_traject, seed_account, state_with_user_token_mode, traject_ref,
    verified_session,
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
use regelrecht_editor_api::corpus_handlers::{list_traject_corpus_laws, list_traject_sources};
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};

use regelrecht_pipeline::test_utils::TestDb;

const TRAJECT_LAW: &str = "wet_traject_eigen";
const SEED_ONLY_LAW: &str = "wet_alleen_centraal";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";
const USER_TOKEN: &str = "user-token";

/// Traject zoals deployed: GitHub writable-own op een user-gekozen repo
/// (auth_ref zonder geconfigureerd `CORPUS_AUTH_*_TOKEN`-env → server-side
/// géén token) plus een lokale read-seed die het centrale corpus speelt.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    own_repo_owner: &str,
    own_repo_name: &str,
    seed_dir: &std::path::Path,
) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 $2, $3, $4, 'main', $5,
                 0, 'example-unset-token-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(own_repo_owner)
    .bind(own_repo_name)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

async fn add_member(pool: &PgPool, traject_id: Uuid, account_id: Uuid) {
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'contributor')",
    )
    .bind(traject_id)
    .bind(account_id)
    .execute(pool)
    .await
    .unwrap();
}

fn write_central_law(central_dir: &std::path::Path, law_id: &str) {
    let law_dir = central_dir.join("wet").join(law_id);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {law_id}\nname: Centrale versie\n"),
    )
    .unwrap();
}

/// De Trees-listing van de traject-repo — alleen geserveerd wanneer de
/// request het gekoppelde user-token draagt. Elke andere (token-loze)
/// request valt door naar wiremock's default 404: GitHub's gedrag op een
/// privé-repo zonder token.
async fn mount_tree_requiring_user_token(server: &MockServer, own_repo: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{own_repo}/git/trees/{OWN_BRANCH}")))
        .and(header("authorization", format!("Bearer {USER_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {
                    "path": format!("{OWN_SUBPATH}/wet/{TRAJECT_LAW}/2025-01-01.yaml"),
                    "type": "blob",
                    "sha": "blob-2025",
                },
            ],
        })))
        .mount(server)
        .await;
}

fn ids_query() -> Query<PaginationParams> {
    Query(PaginationParams {
        offset: 0,
        limit: None,
        q: None,
        ids: Some(format!("{TRAJECT_LAW},{SEED_ONLY_LAW}")),
        source: None,
        enriched: false,
    })
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

/// Eén test voor alle scenario's: de `GITHUB_API_BASE`-override is
/// proces-breed, dus parallelle tests met elk een eigen mockserver zouden
/// elkaars fetchers verhangen.
#[tokio::test]
async fn traject_index_scans_with_user_token_and_fails_loud_without_any_token() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path(), TRAJECT_LAW);
    write_central_law(central.path(), SEED_ONLY_LAW);

    let oauth = GithubOAuth::for_tests(true); // user-token-modus aan
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());

    // Alice heeft haar GitHub-account gekoppeld; Bob (ook lid) niet.
    let (alice_id, alice_sub) = seed_account(&db.pool, "alice@test.local").await;
    let (bob_id, bob_sub) = seed_account(&db.pool, "bob@test.local").await;
    let alice = AccountRecord {
        id: alice_id,
        person_sub: alice_sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };
    let bob = AccountRecord {
        id: bob_id,
        person_sub: bob_sub.clone(),
        email: "bob@test.local".to_string(),
        name: "Test User".to_string(),
    };
    let alice_headers = cookie_headers(&oauth, alice.id);

    // ------------------------------------------------------------------
    // Scenario 1 (happy path): privé repo, alleen leesbaar met Alice's
    // user-token → de scan slaagt via dat token en de traject-wet wint op
    // priority 0. De seed-only wet blijft zichtbaar (federatie).
    // ------------------------------------------------------------------
    let repo_a = "example-org/example-private-repo";
    let traject_a = seeded_traject(
        &db.pool,
        alice_id,
        "example-org",
        "example-private-repo",
        central.path(),
    )
    .await;
    add_member(&db.pool, traject_a, bob_id).await;
    let tref_a = traject_ref(traject_a);
    mount_tree_requiring_user_token(&server, repo_a).await;

    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&alice_sub, &alice.email).await,
        Path(tref_a.clone()),
        ids_query(),
        alice_headers.clone(),
    )
    .await
    .expect("scan via user-token: de bibliotheek moet werken");
    assert_eq!(
        laws.0.len(),
        2,
        "traject-wet + seed-only wet, got {:?}",
        laws.0
    );
    let traject_law = laws.0.iter().find(|l| l.law_id == TRAJECT_LAW).unwrap();
    assert_eq!(
        traject_law.source_id, "traject-own-test",
        "de traject-wet moet uit de traject-eigen source komen, niet de seed"
    );
    assert_eq!(traject_law.source_priority, 0);
    let seed_law = laws.0.iter().find(|l| l.law_id == SEED_ONLY_LAW).unwrap();
    assert_eq!(
        seed_law.source_id, "central-seed",
        "seed-only wetten blijven zichtbaar zodra de traject-scan slaagt"
    );

    let sources = list_traject_sources(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&alice_sub, &alice.email).await,
        Path(tref_a.clone()),
        alice_headers.clone(),
    )
    .await
    .expect("sources-listing moet slagen");
    let own = sources
        .0
        .iter()
        .find(|s| s.id == "traject-own-test")
        .unwrap();
    assert_eq!(own.law_count, 1, "de traject-wet telt mee in de index");
    assert_eq!(own.index_error, None, "gezonde scan → geen index_error");

    // ------------------------------------------------------------------
    // Scenario 2a: de gecachte snapshot (het scan-*resultaat*) is deelbaar
    // — Bob, zonder koppeling, wordt eruit bediend. Metadata is voor alle
    // traject-leden zichtbaar; het token zelf zit nergens in.
    // ------------------------------------------------------------------
    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(bob.clone()),
        verified_session(&bob_sub, &bob.email).await,
        Path(tref_a.clone()),
        ids_query(),
        HeaderMap::new(),
    )
    .await
    .expect("gecachte gezonde snapshot bedient ook ongekoppelde leden");
    assert_eq!(laws.0.len(), 2);

    // ------------------------------------------------------------------
    // Scenario 2b: na een cache-invalidatie moet de server opnieuw
    // scannen. Bob's request draagt geen token en Alice's token is
    // nergens bewaard → de scan faalt (unauthenticated Trees → 404) en de
    // bibliotheek faalt LUID met de 428-koppel-flow — geen stille lijst
    // met alleen seed-wetten.
    // ------------------------------------------------------------------
    state.trajects.invalidate(traject_a).await;
    let err = list_traject_corpus_laws(
        State(state.clone()),
        Extension(bob.clone()),
        verified_session(&bob_sub, &bob.email).await,
        Path(tref_a.clone()),
        ids_query(),
        HeaderMap::new(),
    )
    .await
    .expect_err("zonder enig token mag de bibliotheek NIET stil terugvallen op de seed");
    assert_eq!(
        err.0,
        StatusCode::PRECONDITION_REQUIRED,
        "ongekoppeld in user-token-modus → 428 de koppel-flow in, got: {}: {}",
        err.0,
        err.1
    );

    // Criterium: `index_error` blijft ondertussen zichtbaar op /sources —
    // dat endpoint is de diagnose-route en valt buiten de poort.
    let sources = list_traject_sources(
        State(state.clone()),
        Extension(bob.clone()),
        verified_session(&bob_sub, &bob.email).await,
        Path(tref_a.clone()),
        HeaderMap::new(),
    )
    .await
    .expect("/sources moet ook bij een kapotte scan blijven werken");
    let own = sources
        .0
        .iter()
        .find(|s| s.id == "traject-own-test")
        .unwrap();
    assert_eq!(own.law_count, 0);
    let index_err = own
        .index_error
        .as_deref()
        .expect("gefaalde scan moet een index_error op de source zetten");
    assert!(index_err.contains("404"), "got: {index_err}");

    // ------------------------------------------------------------------
    // Scenario 3 (zelfheling): Alice's volgende request draagt wél een
    // token → de kapotte token-loos-gescande snapshot wordt synchroon
    // herbouwd en de bibliotheek is weer gezond.
    // ------------------------------------------------------------------
    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&alice_sub, &alice.email).await,
        Path(tref_a.clone()),
        ids_query(),
        alice_headers.clone(),
    )
    .await
    .expect("een request mét token moet de kapotte snapshot herbouwen");
    assert_eq!(laws.0.len(), 2);
    assert_eq!(
        laws.0
            .iter()
            .find(|l| l.law_id == TRAJECT_LAW)
            .unwrap()
            .source_priority,
        0
    );

    // ------------------------------------------------------------------
    // Scenario 4: gekoppeld maar géén toegang — de scan faalt ook mét
    // user-token (repo zonder Trees-mock → altijd 404). De diagnose vraagt
    // het na met datzelfde token en krijgt op de repo-lookup óók een 404:
    // de repo staat niet op dit adres. Dus 404 met een melding die het
    // adres noemt — geen 428 (koppelen lost dit niet op) en geen stille
    // seed-fallback.
    // ------------------------------------------------------------------
    let traject_b = seeded_traject(
        &db.pool,
        alice_id,
        "example-org",
        "example-no-access-repo",
        central.path(),
    )
    .await;
    let tref_b = traject_ref(traject_b);

    let err = list_traject_corpus_laws(
        State(state.clone()),
        Extension(alice.clone()),
        verified_session(&alice_sub, &alice.email).await,
        Path(tref_b.clone()),
        ids_query(),
        alice_headers.clone(),
    )
    .await
    .expect_err("scan-fout mét token → expliciete fout, geen stille fallback");
    assert_eq!(
        err.0,
        StatusCode::NOT_FOUND,
        "gekoppeld-maar-geen-toegang is geen koppel-probleem, maar wél een \
         geclassificeerde fout, got: {}: {}",
        err.0,
        err.1
    );
    assert!(
        err.1.contains("example-org/example-no-access-repo"),
        "de melding moet het adres noemen waar de repo niet (meer) staat, got: {}",
        err.1
    );
    assert!(
        !err.1.contains("De gegevens konden niet worden opgehaald"),
        "geen enkele situatie mag terugvallen op de generieke melding, got: {}",
        err.1
    );

    std::env::remove_var("GITHUB_API_BASE");
}
