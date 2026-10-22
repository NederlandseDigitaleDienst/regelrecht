//! Pint de inperkingsregels van [`ScanTokenOverride`] op de indexscan
//! (`index_all_sources_with_override`):
//!
//! 1. het override-token authenticeert de scan van precies de source
//!    waarvoor het is afgegeven (de writable-own van een traject) wanneer
//!    de server géén token voor die source resolvet;
//! 2. het override-token bereikt **nooit** een andere source — de seed
//!    scant met z'n eigen servertoken (de mock matcht exact op dat token,
//!    dus een gelekt user-token zou de seed-scan laten falen);
//! 3. een geconfigureerd servertoken wint van het override-token — de
//!    scan-spiegel van de request-gebonden read-fallback uit PR #955.
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam; alle fases staan daarom in ÉÉN test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use regelrecht_corpus::models::{GitHubSource, Scope, Source, SourceType};
use regelrecht_corpus::{CorpusRegistry, ScanTokenOverride};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWN_BRANCH: &str = "traject-branch";
const SEED_BRANCH: &str = "main";

fn github_source(
    id: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    priority: u32,
    auth_ref: &str,
    strict_auth: bool,
) -> Source {
    Source {
        id: id.to_string(),
        name: id.to_string(),
        source_type: SourceType::GitHub {
            github: GitHubSource {
                owner: owner.to_string(),
                repo: repo.to_string(),
                branch: branch.to_string(),
                path: None,
                git_ref: None,
            },
        },
        scopes: Vec::<Scope>::new(),
        priority,
        auth_ref: Some(auth_ref.to_string()),
        strict_auth,
        enriched: None,
        load_all: false,
    }
}

/// Trees-listing met één wet, alleen geserveerd wanneer de request het
/// verwachte Bearer-token draagt. Elke andere request op dit pad valt
/// door naar wiremock's default 404 — precies GitHub's gedrag op een
/// privé-repo zonder (of met een verkeerd) token.
async fn mount_tree_requiring_token(server: &MockServer, repo: &str, branch: &str, token: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{repo}/git/trees/{branch}")))
        .and(header("authorization", format!("Bearer {token}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {
                    "path": format!("wet/wet_van_{}/2025-01-01.yaml", repo.replace(['/', '-'], "_")),
                    "type": "blob",
                    "sha": "blob-1",
                },
            ],
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn scan_override_reaches_only_its_source_and_loses_from_server_tokens() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());
    // De seed heeft een servertoken; de mock matcht er exact op, dus als
    // de override daar zou lekken (Bearer user-token) faalt de seed-scan.
    std::env::set_var("CORPUS_AUTH_EXAMPLE_SEED_REF_TOKEN", "seed-server-token");

    let own = github_source(
        "traject-own",
        "example-org",
        "example-own-repo",
        OWN_BRANCH,
        0,
        // Geen CORPUS_AUTH_*-env voor deze ref: server resolvet géén token.
        "example-unset-own-ref",
        true,
    );
    let seed = github_source(
        "seed-gh",
        "example-org",
        "example-seed-repo",
        SEED_BRANCH,
        2,
        "example-seed-ref",
        false,
    );
    let registry = CorpusRegistry::from_sources(vec![own, seed]);

    mount_tree_requiring_token(
        &server,
        "example-org/example-own-repo",
        OWN_BRANCH,
        "user-token",
    )
    .await;
    mount_tree_requiring_token(
        &server,
        "example-org/example-seed-repo",
        SEED_BRANCH,
        "seed-server-token",
    )
    .await;

    // ------------------------------------------------------------------
    // Fase 1: zonder override faalt de token-loze writable-own (404); de
    // seed scant gewoon met z'n servertoken.
    // ------------------------------------------------------------------
    let (map, failed) = registry
        .index_all_sources_async(None, "2026-06-01")
        .await
        .unwrap();
    assert_eq!(failed.len(), 1, "alleen de token-loze own hoort te falen");
    assert_eq!(failed[0].source_id, "traject-own");
    assert!(
        failed[0].error.contains("404"),
        "scan-fout moet de onderliggende 404 dragen, got: {}",
        failed[0].error
    );
    assert!(map
        .get_law("wet_van_example_org_example_seed_repo")
        .is_some());

    // ------------------------------------------------------------------
    // Fase 2: met override scant de own via het user-token; de seed
    // blijft op z'n servertoken (mock zou anders niet matchen).
    // ------------------------------------------------------------------
    let (map, failed) = registry
        .index_all_sources_with_override(
            None,
            Some(ScanTokenOverride {
                source_id: "traject-own",
                token: "user-token",
            }),
            "2026-06-01",
        )
        .await
        .unwrap();
    assert!(
        failed.is_empty(),
        "beide sources moeten scannen: {failed:?}"
    );
    assert!(map
        .get_law("wet_van_example_org_example_own_repo")
        .is_some());
    assert!(map
        .get_law("wet_van_example_org_example_seed_repo")
        .is_some());

    // ------------------------------------------------------------------
    // Fase 3: zodra de own-source wél een servertoken heeft, wint dat van
    // de override. De trees-mock voor het servertoken serveert een ANDERE
    // wet dan de user-token-mock, dus het resultaat verraadt welk token
    // de scan droeg.
    // ------------------------------------------------------------------
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/example-org/example-own-repo/git/trees/{OWN_BRANCH}"
        )))
        .and(header("authorization", "Bearer own-server-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha-2",
            "truncated": false,
            "tree": [
                { "path": "wet/wet_via_servertoken/2025-01-01.yaml", "type": "blob", "sha": "blob-2" },
            ],
        })))
        .mount(&server)
        .await;
    std::env::set_var(
        "CORPUS_AUTH_EXAMPLE_UNSET_OWN_REF_TOKEN",
        "own-server-token",
    );

    let (map, failed) = registry
        .index_all_sources_with_override(
            None,
            Some(ScanTokenOverride {
                source_id: "traject-own",
                token: "user-token",
            }),
            "2026-06-01",
        )
        .await
        .unwrap();
    std::env::remove_var("CORPUS_AUTH_EXAMPLE_UNSET_OWN_REF_TOKEN");
    assert!(
        failed.is_empty(),
        "scan met servertoken moet slagen: {failed:?}"
    );
    assert!(
        map.get_law("wet_via_servertoken").is_some(),
        "een geconfigureerd servertoken moet de scan dragen, niet de override"
    );
    assert!(
        map.get_law("wet_van_example_org_example_own_repo")
            .is_none(),
        "de user-token-listing mag niet gebruikt zijn zodra er een servertoken is"
    );

    std::env::remove_var("CORPUS_AUTH_EXAMPLE_SEED_REF_TOKEN");
    std::env::remove_var("GITHUB_API_BASE");
}
