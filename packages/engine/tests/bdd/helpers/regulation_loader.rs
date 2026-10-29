//! Regulation loader for BDD tests
//!
//! Loads all YAML regulation files from the corpus/regulation/nl directory.

use crate::common::regulation_base_paths;
use regelrecht_engine::{EngineError, LawExecutionService};
use walkdir::WalkDir;

/// Load all regulation YAML files into the service.
///
/// Scans the `corpus/regulation/nl/` directory (or the `nl/` of each base in
/// `REGULATION_PATH`) and loads all `.yaml` files found.
pub fn load_all_regulations(service: &mut LawExecutionService) -> Result<usize, EngineError> {
    let mut count = 0;
    for base in regulation_base_paths() {
        count += load_regulations_in(service, &base.join("nl"))?;
    }
    tracing::info!(count = count, "Loaded regulations");
    Ok(count)
}

/// Load every `.yaml` file under `regulation_dir` into the service.
fn load_regulations_in(
    service: &mut LawExecutionService,
    regulation_dir: &std::path::Path,
) -> Result<usize, EngineError> {
    if !regulation_dir.exists() {
        return Err(EngineError::LoadError(format!(
            "Regulation directory not found: {}",
            regulation_dir.display()
        )));
    }

    let mut count = 0;

    for entry in WalkDir::new(regulation_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process YAML files
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            let content = std::fs::read_to_string(path).map_err(|e| {
                EngineError::LoadError(format!("Failed to read {}: {}", path.display(), e))
            })?;

            let content = if crate::reverse_actions::enabled() {
                crate::reverse_actions::reversed_actions(&content).map_err(|e| {
                    EngineError::LoadError(format!("Failed to reverse {}: {}", path.display(), e))
                })?
            } else {
                content
            };

            match service.load_law(&content) {
                Ok(law_id) => {
                    tracing::debug!(law_id = %law_id, path = %path.display(), "Loaded law");
                    count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to load law file (skipping)"
                    );
                    // Continue loading other files even if one fails
                }
            }
        }
    }

    Ok(count)
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
