//! Helpers the cell's tests share.
// Each test crate includes this module and uses part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use regelrecht_cel::identity::REDUCTIONS;
use regelrecht_engine::LawExecutionService;

/// An empty directory for a cell, with only its identity (`cel.yaml`,
/// RFC-052).
pub fn cell_tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("cel.yaml"), "name: Testcel\n").unwrap();
    dir
}

/// The corpus of the repository.
pub fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

/// Load every version in each directory of `dirs`, relative to the corpus
/// (`regulation/nl/wet/<id>`, `cells/<cell>/reductions/<id>`), into
/// `service`.
pub fn load(service: &mut LawExecutionService, dirs: impl IntoIterator<Item = impl AsRef<str>>) {
    for dir in dirs {
        let dir = corpus().join(dir.as_ref());
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                service
                    .load_law(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
    }
}

/// The directory of the reduction `id` in the corpus, relative to it: with
/// the cell that keeps it, or with no cell (`cells/reductions/<id>`).
pub fn reduction_dir(id: &str) -> String {
    find_reduction(id).unwrap_or_else(|| panic!("no reduction '{id}' in the corpus"))
}

fn find_reduction(id: &str) -> Option<String> {
    let shared = format!("cells/{REDUCTIONS}/{id}");
    if corpus().join(&shared).is_dir() {
        return Some(shared);
    }
    std::fs::read_dir(corpus().join("cells"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .map(|cell| format!("cells/{cell}/{REDUCTIONS}/{id}"))
        .find(|dir| corpus().join(dir).is_dir())
}

/// Write `yaml` as the identity of the cell in `dir` (`cel.yaml`), and keep
/// there every reduction of a cell in the corpus its answers name, so the
/// cell keeps the reductions it answers from. A reduction the corpus does
/// not hold the test keeps itself ([`keep_reduction`]); one of no cell it
/// never keeps.
pub fn write_identity(dir: &Path, yaml: &str) {
    std::fs::write(dir.join("cel.yaml"), yaml).unwrap();
    let identity: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    let Some(answers) = identity.get("answers").and_then(|a| a.as_mapping()) else {
        return;
    };
    let shared = format!("cells/{REDUCTIONS}/");
    for id in answers.values().filter_map(|r| r.as_str()) {
        let of_a_cell = find_reduction(id).is_some_and(|found| !found.starts_with(&shared));
        if !id.trim().is_empty() && of_a_cell {
            keep_corpus_reduction(dir, id);
        }
    }
}

/// Keep the reduction `id` in the cell directory `dir`, with `yaml` as its
/// version (`<valid_from>.yaml`).
pub fn keep_reduction(dir: &Path, id: &str, yaml: &str) {
    let valid_from = yaml
        .lines()
        .find_map(|line| line.strip_prefix("valid_from: "))
        .map(|v| v.trim().trim_matches('\'').to_string())
        .unwrap_or_else(|| panic!("reduction '{id}' has no valid_from"));
    let versions = dir.join(REDUCTIONS).join(id);
    std::fs::create_dir_all(&versions).unwrap();
    std::fs::write(versions.join(format!("{valid_from}.yaml")), yaml).unwrap();
}

/// Keep the reduction `id` of the corpus in the cell directory `dir`, with
/// the versions the corpus holds.
pub fn keep_corpus_reduction(dir: &Path, id: &str) {
    let from = corpus().join(reduction_dir(id));
    for entry in std::fs::read_dir(from).unwrap() {
        keep_reduction(
            dir,
            id,
            &std::fs::read_to_string(entry.unwrap().path()).unwrap(),
        );
    }
}
