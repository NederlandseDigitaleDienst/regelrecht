//! Scenario sets: features that run against a chosen set of regelwerken and
//! nothing else.
//!
//! A scenario set is a directory with two parts:
//!
//! ```text
//! <set>/
//!   regulation/   the regelwerken of the set, any tree of `.yaml` files
//!   scenarios/    the features, `*.feature`, also in subdirectories
//! ```
//!
//! A feature under `<set>/scenarios/` runs against the regelwerken under
//! `<set>/regulation/` and nothing else: not the corpus, not the fixtures of
//! `bdd/fixtures`. A hook fires only from a regelwerk the set loads, so in a
//! construct a hook fires only from the set's own regelwerken.
//!
//! The constructs under `bdd/constructs/` are the scenario sets this runner
//! finds itself ([`constructs`]). A corpus laid out the same way (a
//! `regulation/` directory with a `scenarios/` directory beside it) is a
//! scenario set too: its features would load its regelwerken by the same
//! rule, because the set is read from where a feature sits.

use std::path::{Path, PathBuf};

use regelrecht_engine::LawExecutionService;
use walkdir::WalkDir;

use crate::helpers::regulation_loader::regulation_source;

/// The directory of the features of a scenario set.
const SCENARIOS: &str = "scenarios";
/// The directory of the regelwerken of a scenario set.
const REGULATION: &str = "regulation";

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

    /// Load every regelwerk of the set into `service`.
    pub fn load(&self, service: &mut LawExecutionService) {
        for path in files_with_extension(&self.root.join(REGULATION), "yaml") {
            load_regelwerk(service, &path);
        }
    }
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
    let yaml = regulation_source(path).unwrap_or_else(|e| panic!("{e}"));
    if let Err(e) = service.load_law(&yaml) {
        panic!("{}: {e}", path.display());
    }
}
