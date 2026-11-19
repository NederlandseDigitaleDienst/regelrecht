//! Regulation loader for BDD tests
//!
//! Loads all YAML regulation files from the corpus/regulation/nl directory,
//! or from the `nl/` of each directory in `REGULATION_PATH`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::common::regulation_base_paths;
use regelrecht_engine::{EngineError, LawExecutionService};
use walkdir::WalkDir;

/// Load all regulation YAML files into the service.
///
/// Scans the `corpus/regulation/nl/` directory (or the `nl/` of each base in
/// `REGULATION_PATH`) and loads all `.yaml` files found.
///
/// # Errors
///
/// As [`load_regulations_from`].
pub fn load_all_regulations(service: &mut LawExecutionService) -> Result<usize, EngineError> {
    load_regulations_from(service, &regulation_base_paths())
}

/// Load every `.yaml` file under the `nl/` of each base in `bases`, in order.
///
/// The engine keeps one version of a `$id` per `valid_from`: a second file
/// with the same `$id` and `valid_from` replaces the first, and one with
/// another `valid_from` becomes a version beside it. Within one base that is
/// how a regelwerk has its versions. Across two bases (a corpus of its own
/// beside `corpus/regulation`) it would make the outcome depend on the order
/// of `REGULATION_PATH`, or mix the versions of two regelwerken that share a
/// name, without a word. A `$id` that two bases both hold therefore fails the
/// load, naming a file from each.
///
/// # Errors
///
/// When a base has no `nl/` directory, a file cannot be read or does not
/// load, or two bases hold the same `$id`.
pub fn load_regulations_from(
    service: &mut LawExecutionService,
    bases: &[PathBuf],
) -> Result<usize, EngineError> {
    // The base and file each `$id` was first loaded from.
    let mut first_seen: HashMap<String, (&Path, PathBuf)> = HashMap::new();
    let mut count = 0;
    for base in bases {
        for (law_id, path) in load_regulations_in(service, &base.join("nl"))? {
            count += 1;
            let (first_base, first_path) = first_seen
                .entry(law_id.clone())
                .or_insert_with(|| (base.as_path(), path.clone()));
            if *first_base != base.as_path() {
                return Err(EngineError::LoadError(format!(
                    "regelwerk '{law_id}' is in two directories of REGULATION_PATH: \
                     {} and {}; remove it from one of them",
                    first_path.display(),
                    path.display()
                )));
            }
        }
    }
    tracing::info!(count = count, "Loaded regulations");
    Ok(count)
}

/// The YAML of the regelwerk at `path`, as the runners load it: with
/// `BDD_REVERSE_ACTIONS` set, the actions of each article reversed.
///
/// # Errors
///
/// When the file cannot be read or its actions cannot be reversed.
pub fn regulation_source(path: &Path) -> Result<String, EngineError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| EngineError::LoadError(format!("Failed to read {}: {e}", path.display())))?;
    if crate::reverse_actions::enabled() {
        crate::reverse_actions::reversed_actions(&content).map_err(|e| {
            EngineError::LoadError(format!("Failed to reverse {}: {e}", path.display()))
        })
    } else {
        Ok(content)
    }
}

/// Load every `.yaml` file under `regulation_dir` into the service, and
/// return the `$id` and path of each file that loaded.
fn load_regulations_in(
    service: &mut LawExecutionService,
    regulation_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, EngineError> {
    if !regulation_dir.exists() {
        return Err(EngineError::LoadError(format!(
            "Regulation directory not found: {}",
            regulation_dir.display()
        )));
    }

    let mut loaded = Vec::new();

    for entry in WalkDir::new(regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only YAML files, leaving out dot-prefixed sidecars such as
        // `.enrichment.yaml`, which are not a regelwerk.
        let sidecar = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") && !sidecar {
            let content = regulation_source(path)?;
            // A file that does not load fails the run. Skipping it would let a
            // version from another directory of REGULATION_PATH, or no version
            // at all, stand in for it while the scenarios still pass.
            let law_id = service
                .load_law(&content)
                .map_err(|e| EngineError::LoadError(format!("{}: {e}", path.display())))?;
            tracing::debug!(law_id = %law_id, path = %path.display(), "Loaded law");
            loaded.push((law_id, path.to_path_buf()));
        }
    }

    Ok(loaded)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::load_all_regulations;
    use regelrecht_engine::LawExecutionService;

    // Runs in the `bdd_regulation_loader` target; the `bdd` target is
    // `harness = false` and never runs it.
    #[test]
    fn test_specific_laws_loaded() {
        let mut service = LawExecutionService::new();
        let count = load_all_regulations(&mut service).expect("Failed to load regulations");
        assert!(count > 0, "Expected to load at least one regulation");

        // Check that key laws are loaded
        assert!(
            service.has_law("participatiewet"),
            "participatiewet should be loaded"
        );
        assert!(
            service.has_law("burgerlijk_wetboek_boek_5"),
            "BW5 should be loaded"
        );
    }
}
