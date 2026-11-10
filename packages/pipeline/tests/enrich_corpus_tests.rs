//! End-to-end coverage of the base-freshness guard in `create_enrich_corpus`.
//!
//! The pure `decide_base_action` unit tests (in `enrich.rs`) pin the decision
//! logic, and the corpus crate's primitives (`fetch_base_blob_sha`,
//! `is_tracked`, sparse checkout) are tested next to their implementation. This
//! test wires the two together through the real `create_enrich_corpus` glue: it
//! builds a bare git remote whose `enrich/test` branch carries a law plus a
//! `.enrichment.yaml` recording a *stale* `source_hash`, then asserts the guard
//! fails the job loudly with `BaseDrift` instead of silently re-enriching over a
//! possibly-validated result.
//!
//! NOTE: this test is DB-free — it needs only `git` and a temp dir, no
//! testcontainers/Postgres. It lives in the integration `tests/` target (rather
//! than as a `#[cfg(test)]` unit test) purely for filesystem/git isolation: it
//! shells out to real `git` against a bare remote in a `tempfile` dir. It is not
//! Docker-dependent, and it also runs under `just test`, so coverage isn't
//! gated on the Docker-only `pipeline-integration-test` recipe.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use regelrecht_corpus::CorpusConfig;
use regelrecht_pipeline::PipelineError;
use regelrecht_pipeline::enrich::create_enrich_corpus;
use uuid::Uuid;

/// Run a git command in `dir` and assert it succeeded.
async fn run_git(dir: &Path, args: &[&str]) {
    let output = tokio::process::Command::new("git")
        // Zonder hooks: een globale `init.templateDir` zet de hooks van de
        // ontwikkelaar in elke tijdelijke repo en laat de commit falen.
        .arg("-c")
        .arg("core.hooksPath=")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("failed to spawn git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Configure a committer identity on a working clone.
async fn configure_user(repo: &Path) {
    run_git(repo, &["config", "user.name", "test"]).await;
    run_git(repo, &["config", "user.email", "test@test.nl"]).await;
}

/// Write `content` to `rel` inside `repo`, creating parent directories.
async fn write_file(repo: &Path, rel: &str, content: &str) {
    let path = repo.join(rel);
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&path, content).await.unwrap();
}

/// A law that is already enriched on `enrich/test` but whose recorded
/// `source_hash` no longer matches the base blob (the base moved after the
/// enrichment was recorded) must fail as `BaseDrift`, never be re-enriched.
#[tokio::test]
async fn create_enrich_corpus_fails_on_drifted_base() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // 1. Bare remote with a `development` branch carrying one law YAML.
    let bare = root.join("bare.git");
    run_git(
        root,
        &[
            "init",
            "--bare",
            "--initial-branch=development",
            bare.to_str().unwrap(),
        ],
    )
    .await;
    let bare_url = format!("file://{}", bare.display());

    let law_path = "regulation/nl/wet/test_law/2025-01-01.yaml";
    let meta_path = "regulation/nl/wet/test_law/.enrichment.yaml";
    let law_content = "$id: test_law\narticles: []\n";

    // Seed `development` through a working clone.
    let seed = root.join("seed");
    run_git(root, &["clone", &bare_url, seed.to_str().unwrap()]).await;
    configure_user(&seed).await;
    write_file(&seed, law_path, law_content).await;
    run_git(&seed, &["add", "."]).await;
    run_git(&seed, &["commit", "-m", "harvest law"]).await;
    run_git(&seed, &["push", "origin", "development"]).await;

    // 2. `enrich/test`: the same law bytes plus a `.enrichment.yaml` whose
    //    `source_hash` is a stale placeholder that does NOT equal the base blob
    //    SHA of the law — simulating a base that moved after enrichment.
    run_git(&seed, &["checkout", "-b", "enrich/test"]).await;
    let stale_meta = "law_id: test_law\n\
         provider: claude\n\
         source_hash: \"0000000000000000000000000000000000000000\"\n";
    write_file(&seed, meta_path, stale_meta).await;
    run_git(&seed, &["add", "."]).await;
    run_git(&seed, &["commit", "-m", "enrich law with stale provenance"]).await;
    run_git(&seed, &["push", "-u", "origin", "enrich/test"]).await;

    // 3. base_config: the worker's own base branch is `development`.
    let mut base_config = CorpusConfig::new(&bare_url, root.join("base-checkout"));
    base_config.branch = "development".into();

    // 4. Run the real enrichment-checkout glue against `enrich/test`. It clones
    //    the enrich branch (sparse), fetches the base blob SHA, reads the stored
    //    provenance, and applies the freshness decision.
    let result = create_enrich_corpus(&base_config, "enrich/test", Uuid::new_v4(), law_path).await;

    // 5. Stale provenance -> a loud, terminal failure (requires human
    //    re-enrich; handled via fail_job_terminal, never retried), never a
    //    silent overwrite.
    match result {
        Err(PipelineError::BaseDrift {
            yaml_path,
            base,
            expected,
            actual,
        }) => {
            assert_eq!(yaml_path, law_path);
            assert_eq!(base, "development");
            assert_eq!(expected, "0000000000000000000000000000000000000000");
            assert_ne!(
                actual, expected,
                "actual base blob SHA must differ from the stale recorded one"
            );
        }
        Err(other) => panic!("expected BaseDrift, got error: {other:?}"),
        Ok(_) => panic!("expected BaseDrift, but create_enrich_corpus succeeded"),
    }
}
