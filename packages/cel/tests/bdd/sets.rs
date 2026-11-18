//! Scenario sets: features that run against a chosen set of regelwerken and
//! nothing else.
//!
//! A scenario set is a directory with two parts and an optional manifest:
//!
//! ```text
//! <set>/
//!   regulation/   the regelwerken of the set, any tree of `.yaml` files
//!   scenarios/    the features, `*.feature`, also in subdirectories
//!   corpus.yaml   optional: regelwerken from the corpus it loads as well
//! ```
//!
//! A feature under `<set>/scenarios/` runs against the regelwerken under
//! `<set>/regulation/` and those `corpus.yaml` names, and nothing else: not
//! the rest of the corpus, not the fixtures of `bdd/fixtures`. A hook fires
//! only from a regelwerk the set loads, so a construct is tried against the
//! real Awb, say, without every other law of the corpus taking part.
//!
//! `corpus.yaml` names regelwerken by `$id` under `regelwerken:`. Each is
//! read from the corpus (`REGULATION_PATH`, or `corpus/regulation`), every
//! version in its directory, and is never copied into the set.
//!
//! The constructs under `bdd/constructs/` are the scenario sets this runner
//! finds itself ([`constructs`]). A corpus laid out the same way (a
//! `regulation/` directory with a `scenarios/` directory beside it) is a
//! scenario set too: its features would load its regelwerken by the same
//! rule, because the set is read from where a feature sits.

use std::path::{Path, PathBuf};

use regelrecht_engine::LawExecutionService;
use serde::Deserialize;
use walkdir::WalkDir;

/// The directory of the features of a scenario set.
const SCENARIOS: &str = "scenarios";
/// The directory of the regelwerken of a scenario set.
const REGULATION: &str = "regulation";
/// The manifest of the regelwerken a scenario set takes from the corpus.
const CORPUS_MANIFEST: &str = "corpus.yaml";

/// `corpus.yaml`: the regelwerken from the corpus a set loads, by `$id`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusManifest {
    regelwerken: Vec<String>,
}

/// A directory with `regulation/` and `scenarios/` (module docs).
pub struct ScenarioSet {
    root: PathBuf,
}

impl ScenarioSet {
    /// The set at `root`, when it holds both parts.
    fn at(root: &Path) -> Option<Self> {
        (root.join(REGULATION).is_dir() && root.join(SCENARIOS).is_dir()).then(|| Self {
            root: root.to_path_buf(),
        })
    }

    /// The set whose `scenarios/` directory holds the feature at `feature`,
    /// at any depth. `None` for a feature outside every set.
    pub fn containing(feature: &Path) -> Option<Self> {
        feature
            .ancestors()
            .skip(1)
            .filter(|dir| dir.file_name().is_some_and(|name| name == SCENARIOS))
            .find_map(|scenarios| scenarios.parent().and_then(Self::at))
    }

    /// Every feature of the set, sorted.
    pub fn features(&self) -> Vec<PathBuf> {
        let mut features = files_with_extension(&self.root.join(SCENARIOS), "feature");
        features.sort();
        features
    }

    /// Load every regelwerk of the set into `service`: those the manifest
    /// names from the corpus, then its own.
    pub fn load(&self, service: &mut LawExecutionService) {
        for id in self.corpus_ids() {
            for path in corpus_versions(&id) {
                load_regelwerk(service, &path);
            }
        }
        for path in files_with_extension(&self.root.join(REGULATION), "yaml") {
            load_regelwerk(service, &path);
        }
    }

    /// The `$id`s `corpus.yaml` names; none when the set has no manifest.
    fn corpus_ids(&self) -> Vec<String> {
        let path = self.root.join(CORPUS_MANIFEST);
        if !path.is_file() {
            return Vec::new();
        }
        let yaml =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let manifest: CorpusManifest =
            serde_yaml_ng::from_str(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        manifest.regelwerken
    }
}

/// Every version of the regelwerk `id` in the corpus: the `.yaml` files in
/// the directory named after it, under `nl/` of each corpus base. Fails when
/// the corpus holds none, so a misspelt `$id` cannot leave a set without it.
fn corpus_versions(id: &str) -> Vec<PathBuf> {
    let mut versions: Vec<PathBuf> = crate::common::regulation_base_paths()
        .iter()
        .flat_map(|base| {
            WalkDir::new(base.join("nl"))
                .into_iter()
                .filter_map(Result::ok)
        })
        .filter(|entry| entry.file_type().is_dir() && entry.file_name() == id)
        .flat_map(|dir| files_with_extension(dir.path(), "yaml"))
        .collect();
    assert!(!versions.is_empty(), "no regelwerk '{id}' in the corpus");
    versions.sort();
    versions
}

/// The construct sets under `constructs`: each subdirectory, sorted. A
/// subdirectory that is no scenario set, or one without a feature, fails the
/// run, so a misspelt `scenarios/` cannot drop a construct silently.
pub fn constructs(constructs: &Path) -> Vec<ScenarioSet> {
    let mut roots: Vec<PathBuf> = std::fs::read_dir(constructs)
        .unwrap_or_else(|e| panic!("{}: {e}", constructs.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("{}: {e}", constructs.display()))
                .path()
        })
        .filter(|path| path.is_dir())
        .collect();
    roots.sort();
    roots
        .iter()
        .map(|root| {
            let set = ScenarioSet::at(root).unwrap_or_else(|| {
                panic!(
                    "{}: a construct holds {REGULATION}/ and {SCENARIOS}/",
                    root.display()
                )
            });
            assert!(
                !set.features().is_empty(),
                "{}: no feature files in {SCENARIOS}/",
                root.display()
            );
            set
        })
        .collect()
}

/// The files under `dir` with extension `extension`, leaving out
/// dot-prefixed sidecars such as `.enrichment.yaml`.
fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.is_file() && path.extension().is_some_and(|x| x == extension))
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect()
}

/// Load the regelwerk at `path` into `service`, failing the scenario when it
/// does not load. As the corpus loader does, `BDD_REVERSE_ACTIONS` reverses
/// the actions of each article first.
pub fn load_regelwerk(service: &mut LawExecutionService, path: &Path) {
    let mut yaml =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if crate::reverse_actions::enabled() {
        yaml = crate::reverse_actions::reversed_actions(&yaml)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    service
        .load_law(&yaml)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}
