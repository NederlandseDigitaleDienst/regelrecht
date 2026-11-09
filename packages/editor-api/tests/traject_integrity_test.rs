//! Integriteitsrapport over de eigen repo van een traject, end-to-end tegen
//! een nagespeelde GitHub.
//!
//! De pure checks zijn per stuk gedekt door de unit-tests in
//! `traject_integrity.rs`; wat daar niet in past is de **bekabeling**:
//!
//! * met welk credential de scan de repo opsomt. Een writable-own met een
//!   geconfigureerd service-token is "writable at rest", en de per-request
//!   tokenresolutie antwoordt dan bewust `None` — juist omdat de backend het
//!   token zelf draagt. De scan praat langs de backend heen rechtstreeks met
//!   GitHub, dus die moet dat server-token opnieuw resolven; doet hij dat
//!   niet, dan somt hij een privé-repo anoniem op en faalt elke aanroep met
//!   een 502. Deze test pint dat vast: de Trees- en Contents-mocks matchen
//!   alléén op `Bearer <service-token>`.
//! * dat een verwijzing naar een wet uit een *andere* bron van het traject
//!   (de seed) gewoon resolvet, en de mapnaam-afwijking wél wordt gemeld.
//! * dat een tweede aanroep zonder nieuwe commits geen enkele body opnieuw
//!   leest (de memo op blob-sha).
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam en het service-token komt uit een env var — beide
//! proces-breed, dus alle scenario's staan in ÉÉN test (zelfde afweging als
//! `service_token_write_precedence_test.rs`). Alle namen zijn fictief: dit is
//! een publieke repo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, insert_traject, seed_account, session_for, state_with_user_token_mode,
    traject_ref,
};

use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::github_oauth::GithubOAuth;
use regelrecht_editor_api::traject_integrity::{get_traject_integrity, FindingKind};

use regelrecht_pipeline::test_utils::TestDb;

const OWN_REPO: &str = "example-org/regelrecht-corpus-example";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";
const SERVICE_AUTH_REF: &str = "example-integrity-token-ref";
const SERVICE_TOKEN_ENV: &str = "CORPUS_AUTH_EXAMPLE_INTEGRITY_TOKEN_REF_TOKEN";
const SERVICE_TOKEN: &str = "service-token";

/// De wet met een kloppende map: mapnaam == `$id`, bestandsnaam ==
/// `valid_from`, en een verwijzing naar een wet uit de seed-bron.
const CLEAN_PATH: &str = "wet/wet_alpha/2025-01-01.yaml";
const CLEAN_BODY: &str = "\
$id: wet_alpha
valid_from: '2025-01-01'
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: amount
            source:
              regulation: wet_seed_voorbeeld
              output: grondslag
";

/// De wet met de afwijking waar de pagina om begonnen is: de map heet
/// `wet_beta`, de YAML declareert `wet_beta_afwijkend`.
const MISMATCH_PATH: &str = "wet/wet_beta/2025-01-01.yaml";
const MISMATCH_BODY: &str = "\
$id: wet_beta_afwijkend
valid_from: '2025-01-01'
";

const SEED_LAW_ID: &str = "wet_seed_voorbeeld";

/// Traject met een GitHub writable-own (service-token via `auth_ref`) plus
/// een lokale seed als centraal corpus.
async fn seeded_traject(pool: &PgPool, owner_id: Uuid, seed_dir: &std::path::Path) -> Uuid {
    let (gh_owner, gh_repo) = OWN_REPO.split_once('/').unwrap();
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 $2, $3, $4, 'main', $5,
                 0, $6, TRUE)",
    )
    .bind(traject_id)
    .bind(gh_owner)
    .bind(gh_repo)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .bind(SERVICE_AUTH_REF)
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

/// De wet die het "centrale corpus" bijdraagt — het doel van de
/// `source.regulation`-verwijzing in `CLEAN_BODY`.
fn write_seed_law(seed_dir: &std::path::Path) {
    let law_dir = seed_dir.join("wet").join(SEED_LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {SEED_LAW_ID}\nname: Seed-versie\nvalid_from: '2025-01-01'\n"),
    )
    .unwrap();
}

/// Branch-check bij het opzetten van de backend: de traject-branch bestaat
/// al, dus geen bootstrap-flow.
async fn mount_branch_exists(server: &MockServer) {
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
}

/// Trees-listing van de traject-branch, mét blob-sha per bestand (de memo
/// hangt eraan) — alleen leesbaar mét het service-token.
async fn mount_tree(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/git/trees/{OWN_BRANCH}")))
        .and(header("authorization", format!("Bearer {SERVICE_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {"path": OWN_SUBPATH, "type": "tree"},
                {"path": format!("{OWN_SUBPATH}/{CLEAN_PATH}"), "type": "blob", "sha": "blob-alpha"},
                {"path": format!("{OWN_SUBPATH}/{MISMATCH_PATH}"), "type": "blob", "sha": "blob-beta"},
                {"path": "README.md", "type": "blob", "sha": "blob-readme"},
            ],
        })))
        .mount(server)
        .await;
}

/// Eén law-body, eveneens alleen met het service-token te lezen.
async fn mount_body(server: &MockServer, relative_path: &str, body: &'static str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO}/contents/{OWN_SUBPATH}/{relative_path}"
        )))
        .and(header("authorization", format!("Bearer {SERVICE_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Hoeveel body-reads (Contents-GETs) wiremock tot nu toe zag.
async fn contents_requests(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| {
            r.url
                .path()
                .starts_with(&format!("/repos/{OWN_REPO}/contents/"))
        })
        .count()
}

#[tokio::test]
async fn integrity_scan_reads_with_the_service_token_and_memoises_bodies() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());
    std::env::set_var(SERVICE_TOKEN_ENV, SERVICE_TOKEN);

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_seed_law(central.path());

    // User-token-modus AAN én een gebruiker zónder gekoppeld GitHub-account:
    // een writable-own met service-token mag daar niet op stuklopen.
    let oauth = GithubOAuth::for_tests(true);
    let state = state_with_user_token_mode(db.pool.clone(), oauth);

    let (owner_id, sub) = seed_account(&db.pool, "alice@test.local").await;
    let account = AccountRecord {
        id: owner_id,
        person_sub: sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };

    let traject_id = seeded_traject(&db.pool, owner_id, central.path()).await;
    let tref = traject_ref(traject_id);

    mount_branch_exists(&server).await;
    mount_tree(&server).await;
    mount_body(&server, CLEAN_PATH, CLEAN_BODY).await;
    mount_body(&server, MISMATCH_PATH, MISMATCH_BODY).await;

    let session = session_for(&sub).await;
    let report = get_traject_integrity(
        State(state.clone()),
        session.clone(),
        Extension(account.clone()),
        Path(tref.clone()),
        HeaderMap::new(),
    )
    .await
    .expect("integriteitsrapport moet met het service-token te bouwen zijn")
    .0;

    // Twee wetbestanden nagekeken; de README telt niet mee.
    assert_eq!(report.checked_laws, 2);
    assert_eq!(report.checked_scenarios, 0);

    // Precies één bevinding: de mapnaam die van het `$id` afwijkt. De
    // verwijzing naar de seed-wet resolvet via de federatie-index, dus die
    // levert géén tweede (gevolg-)bevinding op.
    let kinds: Vec<FindingKind> = report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, vec![FindingKind::DirectoryNameMismatch]);
    let finding = &report.findings[0];
    assert_eq!(finding.law_id.as_deref(), Some("wet_beta_afwijkend"));
    assert_eq!(
        finding.path.as_deref(),
        Some("corpus/regulation/wet/wet_beta")
    );

    let after_first = contents_requests(&server).await;
    assert_eq!(after_first, 2, "koude scan leest beide bodies");

    // Tweede aanroep zonder nieuwe commits: dezelfde blob-sha's, dus de memo
    // dekt alles en er gaat geen enkele body opnieuw over de lijn.
    let again = get_traject_integrity(
        State(state),
        session,
        Extension(account),
        Path(tref),
        HeaderMap::new(),
    )
    .await
    .expect("tweede aanroep moet ook slagen")
    .0;
    assert_eq!(again.findings.len(), 1);
    assert_eq!(
        contents_requests(&server).await,
        after_first,
        "een herhaalde scan op ongewijzigde blob-sha's leest geen bodies opnieuw"
    );
}
