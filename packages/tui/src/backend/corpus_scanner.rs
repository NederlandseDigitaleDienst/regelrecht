use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CorpusNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

/// Law header metadata. Re-exported from the shared `regelrecht-law-model`
/// crate so the tolerant header parser is defined in exactly one place.
pub use regelrecht_law_model::LawHeader as LawMetadata;

/// Scan the corpus directory and return a flat list of nodes for tree rendering.
pub fn scan_corpus(corpus_root: &Path) -> Vec<CorpusNode> {
    let mut nodes = Vec::new();

    let regulation_dir = find_regulation_dir(corpus_root);
    let base = match regulation_dir {
        Some(ref d) => d.as_path(),
        None => return nodes,
    };

    let base_depth = base.components().count();

    for entry in WalkDir::new(base).sort_by_file_name().into_iter().flatten() {
        let path = entry.path().to_path_buf();
        let depth = path.components().count().saturating_sub(base_depth);

        // Skip the root itself
        if depth == 0 {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        nodes.push(CorpusNode {
            path,
            name,
            depth: depth.saturating_sub(1),
            is_dir: entry.file_type().is_dir(),
        });
    }

    nodes
}

/// Extract metadata from a YAML law file without full deserialization.
///
/// Delegates to the shared tolerant header parser in `regelrecht-law-model`.
pub fn extract_metadata(content: &str) -> LawMetadata {
    regelrecht_law_model::parse_law_header(content)
}

/// Find the corpus regulation directory by checking common locations.
fn find_regulation_dir(project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        project_root.join("corpus/regulation/nl"),
        project_root.join("corpus/regulation"),
        project_root.join("corpus/central/nl"),
        project_root.join("corpus/central"),
    ];

    for candidate in &candidates {
        if candidate.is_dir() {
            return Some(candidate.clone());
        }
    }
    None
}

/// Get all YAML file paths from the corpus.
pub fn corpus_yaml_files(project_root: &Path) -> Vec<PathBuf> {
    let regulation_dir = match find_regulation_dir(project_root) {
        Some(d) => d,
        None => return Vec::new(),
    };

    WalkDir::new(regulation_dir)
        .sort_by_file_name()
        .into_iter()
        .flatten()
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn corpus() -> std::io::Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        let wet = root.path().join("corpus/regulation/nl/wet/een_wet");
        fs::create_dir_all(&wet)?;
        fs::write(wet.join("2025-01-01.yaml"), "$id: een_wet\n")?;
        fs::write(wet.join("README.md"), "not a law")?;
        Ok(root)
    }

    #[test]
    fn scan_lists_the_tree_below_the_regulation_dir_with_depths() -> std::io::Result<()> {
        let root = corpus()?;
        let nodes: Vec<_> = scan_corpus(root.path())
            .into_iter()
            .map(|n| (n.name, n.depth, n.is_dir))
            .collect();
        assert_eq!(
            nodes,
            [
                ("wet".to_string(), 0, true),
                ("een_wet".to_string(), 1, true),
                ("2025-01-01.yaml".to_string(), 2, false),
                ("README.md".to_string(), 2, false),
            ]
        );
        Ok(())
    }

    #[test]
    fn yaml_files_skips_everything_that_is_not_yaml() -> std::io::Result<()> {
        let root = corpus()?;
        let files = corpus_yaml_files(root.path());
        assert_eq!(
            files,
            [root
                .path()
                .join("corpus/regulation/nl/wet/een_wet/2025-01-01.yaml")]
        );
        Ok(())
    }

    #[test]
    fn a_project_without_a_corpus_scans_empty() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        assert!(scan_corpus(root.path()).is_empty());
        assert!(corpus_yaml_files(root.path()).is_empty());
        Ok(())
    }
}
