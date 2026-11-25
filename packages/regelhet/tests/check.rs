//! `regelrecht-regelhet check`: a case that starts passes with what it is
//! about, and one the server would refuse at start fails with the reason,
//! from the library and from the binary itself.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use regelrecht_regelhet::app::StartupError;
use regelrecht_regelhet::case::CaseError;
use regelrecht_regelhet::check::{CheckError, check};
use regelrecht_regelhet::config::DEFAULT_MANIFEST;

const BINARY: &str = env!("CARGO_BIN_EXE_regelrecht-regelhet");

/// The root of the repository, where the crate's manifest points.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A copy of the crate's case under `root`, with the manifest at the top as
/// in a manifest repository: the manifest's own text, with its directories
/// copied beside it. Returns the path of the manifest.
fn copy_case(root: &Path) -> PathBuf {
    let text = std::fs::read_to_string(DEFAULT_MANIFEST).unwrap();
    let manifest: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    // Every file the manifest names at its top (the UI spec, the message
    // spec and whatever it names later), and each of its directories.
    let mut paths: Vec<String> = manifest
        .as_mapping()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.as_str() != Some("repo_root"))
        .filter_map(|(_, value)| value.as_str().map(str::to_string))
        .collect();
    for (_, to) in manifest["directories"].as_mapping().unwrap() {
        paths.push(to.as_str().unwrap().to_string());
    }
    for path in &paths {
        let from = repo_root().join(path);
        if from.is_dir() {
            copy_dir(&from, &root.join(path));
        } else {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::copy(&from, root.join(path)).unwrap();
        }
    }
    let target = root.join("bron.yaml");
    std::fs::write(&target, text.replace("repo_root: ../..", "repo_root: .")).unwrap();
    target
}

#[tokio::test]
async fn the_parking_card_starts_and_says_what_it_is() {
    let summary = check(Path::new(DEFAULT_MANIFEST), None).await.unwrap();
    assert_eq!(summary.id, "gehandicaptenparkeerkaart");
    assert_eq!(summary.executor, "gemeente_wilgendaal");
    assert_eq!(summary.cells[0], "gemeente_wilgendaal");
    assert!(summary.cells.contains(&"ggd_wilgendaal".to_string()));
    let shown = summary.to_string();
    assert!(
        shown.contains("case:       gehandicaptenparkeerkaart"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_case_in_a_directory_with_spaces_and_accents_starts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("casus van Wilgendaal é");
    let manifest = copy_case(&root);
    let summary = check(&manifest, None).await.unwrap();
    assert_eq!(summary.id, "gehandicaptenparkeerkaart");
}

#[tokio::test]
async fn a_manifest_that_is_not_there_does_not_read() {
    let err = check(Path::new("/nonexistent/bron.yaml"), None)
        .await
        .unwrap_err();
    assert!(matches!(err, CheckError::Case(_)), "{err:?}");
    assert!(
        err.to_string().contains("the manifest does not read"),
        "{err}"
    );
}

#[tokio::test]
async fn a_case_file_that_is_no_yaml_does_not_load() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = copy_case(dir.path());
    let case = dir
        .path()
        .join("packages/regelhet/casussen/gehandicaptenparkeerkaart.yaml");
    std::fs::write(&case, "casus: [niet afgesloten\n").unwrap();
    let err = check(&manifest, None).await.unwrap_err();
    assert!(
        matches!(
            err,
            CheckError::Case(CaseError::Source(_) | CaseError::Cases(_))
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn of_several_cases_the_one_named_is_checked() {
    let several = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/meerdere-casussen/bron.yaml");
    let err = check(&several, None).await.unwrap_err();
    assert!(
        matches!(err, CheckError::Case(CaseError::NoChoice(_))),
        "{err:?}"
    );
    let summary = check(&several, Some("bedrijfsparkeervergunning"))
        .await
        .unwrap();
    assert_eq!(summary.id, "bedrijfsparkeervergunning");
    let err = check(&several, Some("parkeren")).await.unwrap_err();
    assert!(
        matches!(err, CheckError::Case(CaseError::UnknownCase { .. })),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_ui_spec_the_server_refuses_fails_the_check() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = copy_case(dir.path());
    let spec = dir.path().join("packages/regelhet/ui_spec.yaml");
    let text = std::fs::read_to_string(&spec).unwrap();
    let prefilled = text.replacen("fill: {by: system, login: naam}", "fill: {from: naam}", 1);
    assert_ne!(
        text, prefilled,
        "the UI spec no longer fills the name from the login"
    );
    std::fs::write(&spec, prefilled).unwrap();
    let err = check(&manifest, None).await.unwrap_err();
    assert!(
        matches!(err, CheckError::Startup(StartupError::Service(_))),
        "{err:?}"
    );
}

#[test]
fn the_binary_exits_0_on_a_case_that_starts_and_1_on_one_that_does_not() {
    let ok = Command::new(BINARY)
        .args(["check", DEFAULT_MANIFEST])
        .output()
        .unwrap();
    assert_eq!(ok.status.code(), Some(0), "{ok:?}");
    let stdout = String::from_utf8(ok.stdout).unwrap();
    assert!(stdout.contains("ok: "), "{stdout}");
    assert!(stdout.contains("gehandicaptenparkeerkaart"), "{stdout}");

    let missing = Command::new(BINARY)
        .args(["check", "/nonexistent/bron.yaml"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1), "{missing:?}");
    let stderr = String::from_utf8(missing.stderr).unwrap();
    assert!(stderr.contains("the manifest does not read"), "{stderr}");
}

#[test]
fn the_binary_exits_2_on_a_check_without_a_manifest() {
    let out = Command::new(BINARY).arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8(out.stderr).unwrap().contains("usage:"));
}
