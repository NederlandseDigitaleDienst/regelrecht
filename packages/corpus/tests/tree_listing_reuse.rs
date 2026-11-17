//! Reusing a repository's tree listing across index scans
//! ([`regelrecht_corpus::github::TreeListings`]): a scan asks GitHub which
//! commit the source's ref points at, and lists the tree again only when
//! that commit moved. Listing the central corpus costs seconds, and every
//! traject build and index refresh scans it.
//!
//! GitHub is played by wiremock; the scans reach it through the
//! [`GithubSettings`] each test passes, which also holds the shared
//! listings.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use regelrecht_corpus::models::{GitHubSource, Scope, Source, SourceType};
use regelrecht_corpus::{CorpusRegistry, GithubSettings, ScanTokenOverride};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REPO: &str = "example-org/example-corpus";
const BRANCH: &str = "traject/voorbeeld";
const TODAY: &str = "2026-06-01";

fn source() -> Source {
    Source {
        id: "own".to_string(),
        name: "own".to_string(),
        source_type: SourceType::GitHub {
            github: GitHubSource {
                owner: "example-org".to_string(),
                repo: "example-corpus".to_string(),
                branch: BRANCH.to_string(),
                path: None,
                git_ref: None,
            },
        },
        scopes: Vec::<Scope>::new(),
        priority: 0,
        auth_ref: Some("example-unset-ref".to_string()),
        strict_auth: true,
        enriched: None,
        load_all: false,
        library_only: false,
    }
}

fn settings(server: &MockServer) -> GithubSettings {
    GithubSettings::new(Some(&server.uri()), None, |_| None)
}

/// The branch points at `commit`, for at most `times` requests.
async fn mount_commit(server: &MockServer, commit: &str, times: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/{BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(commit))
        .up_to_n_times(times)
        .mount(server)
        .await;
}

/// The tree at `commit` holds one law, `law_id`; listed exactly `expected`
/// times.
async fn mount_tree(server: &MockServer, commit: &str, law_id: &str, expected: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/trees/{commit}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": format!("tree-of-{commit}"),
            "truncated": false,
            "tree": [
                { "path": format!("wet/{law_id}/2025-01-01.yaml"), "type": "blob", "sha": "blob-1" },
            ],
        })))
        .expect(expected)
        .mount(server)
        .await;
}

#[tokio::test]
async fn an_unchanged_commit_reuses_the_listing() {
    let server = MockServer::start().await;
    mount_commit(&server, "commit-1", 2).await;
    mount_tree(&server, "commit-1", "wet_een", 1).await;
    let github = settings(&server);
    let registry = CorpusRegistry::from_sources(vec![source()]);

    for _ in 0..2 {
        let (map, failed) = registry
            .index_all_sources_async(&github, TODAY)
            .await
            .unwrap();
        assert!(failed.is_empty(), "{failed:?}");
        assert!(map.get_law("wet_een").is_some());
    }
    // `expect(1)` on the tree: the second scan asked for the commit only.
}

#[tokio::test]
async fn a_moved_commit_lists_the_tree_again() {
    let server = MockServer::start().await;
    mount_commit(&server, "commit-1", 1).await;
    mount_commit(&server, "commit-2", 1).await;
    mount_tree(&server, "commit-1", "wet_een", 1).await;
    mount_tree(&server, "commit-2", "wet_twee", 1).await;
    let github = settings(&server);
    let registry = CorpusRegistry::from_sources(vec![source()]);

    let (first, _) = registry
        .index_all_sources_async(&github, TODAY)
        .await
        .unwrap();
    assert!(first.get_law("wet_een").is_some());

    let (second, failed) = registry
        .index_all_sources_async(&github, TODAY)
        .await
        .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
    assert!(second.get_law("wet_twee").is_some());
    assert!(
        second.get_law("wet_een").is_none(),
        "the old listing is gone"
    );
}

#[tokio::test]
async fn a_caller_who_cannot_read_the_commit_gets_no_listing() {
    // A private repository: only `token-a` reads it. Its listing is kept,
    // but reusing it takes the commit, which GitHub does not show to
    // `token-b`. That scan fails as it would without the cache.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/{BRANCH}")))
        .and(header("authorization", "Bearer token-a"))
        .respond_with(ResponseTemplate::new(200).set_body_string("commit-1"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/trees/commit-1")))
        .and(header("authorization", "Bearer token-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-of-commit-1",
            "truncated": false,
            "tree": [
                { "path": "wet/wet_prive/2025-01-01.yaml", "type": "blob", "sha": "blob-1" },
            ],
        })))
        .mount(&server)
        .await;
    let github = settings(&server);
    let registry = CorpusRegistry::from_sources(vec![source()]);
    let scan_as = |token| ScanTokenOverride {
        source_id: "own",
        token,
    };

    let (map, failed) = registry
        .index_all_sources_with_override(&github, Some(scan_as("token-a")), TODAY)
        .await
        .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
    assert!(map.get_law("wet_prive").is_some());

    let (map, failed) = registry
        .index_all_sources_with_override(&github, Some(scan_as("token-b")), TODAY)
        .await
        .unwrap();
    assert_eq!(failed.len(), 1, "the scan without access fails");
    assert!(map.get_law("wet_prive").is_none(), "no listing for token-b");
}

#[tokio::test]
async fn a_ref_without_a_commit_is_listed_by_ref_as_before() {
    // When GitHub resolves no commit for the ref (422), the scan lists the
    // tree by ref, the way it did before listings were kept: a branch that
    // does exist is indexed, and one that does not fails with the Trees
    // 404 that the index diagnosis and the logs expect.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/commits/{BRANCH}")))
        .respond_with(ResponseTemplate::new(422))
        .mount(&server)
        .await;
    let github = settings(&server);
    let registry = CorpusRegistry::from_sources(vec![source()]);

    let (_, failed) = registry
        .index_all_sources_async(&github, TODAY)
        .await
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].error.contains("404"), "{}", failed[0].error);

    mount_tree(&server, BRANCH, "wet_op_de_branch", 1).await;
    let (map, failed) = registry
        .index_all_sources_async(&github, TODAY)
        .await
        .unwrap();
    assert!(failed.is_empty(), "{failed:?}");
    assert!(map.get_law("wet_op_de_branch").is_some());
}
