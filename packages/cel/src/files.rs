//! Where a cell reads its directory from: the file system ([`Disk`]), or a
//! set of files held in memory ([`Bundle`]), such as a browser gets them
//! with the page. A cell reads its identity and catalogue (`cel.yaml`)
//! through [`Files`]; its reductions are rulework of the corpus, and what it
//! records goes to its chronicle, not through here.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Read-only access to files and directories by path.
pub trait Files: std::fmt::Debug + Send + Sync {
    /// The text of the file at `path`.
    fn read(&self, path: &Path) -> io::Result<String>;

    /// The paths of the entries directly in the directory `dir`, in no
    /// particular order. A `dir` that is no directory is an error.
    fn entries(&self, dir: &Path) -> io::Result<Vec<PathBuf>>;

    fn is_dir(&self, path: &Path) -> bool;

    fn is_file(&self, path: &Path) -> bool;

    fn exists(&self, path: &Path) -> bool {
        self.is_dir(path) || self.is_file(path)
    }
}

/// The file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct Disk;

impl Files for Disk {
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn entries(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        std::fs::read_dir(dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

/// Files held in memory, by relative path. A directory is every path that
/// holds a file under it; there are no empty directories.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    files: BTreeMap<PathBuf, String>,
}

impl Bundle {
    /// The bundle of `files`, each a relative path with its text. A path
    /// that is absolute, holds `.` or `..`, or is given twice is an error:
    /// it would not name one place in the bundle.
    pub fn new(files: impl IntoIterator<Item = (String, String)>) -> io::Result<Self> {
        let mut bundle = BTreeMap::new();
        for (path, text) in files {
            let normal = Path::new(&path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
            if path.is_empty() || !normal {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("'{path}': a path in a bundle is relative, without . or .."),
                ));
            }
            if bundle.insert(PathBuf::from(&path), text).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("'{path}' is twice in the bundle"),
                ));
            }
        }
        // A path is a file or a directory, never both, as on a disk.
        let mut paths = bundle.keys().peekable();
        while let Some(path) = paths.next() {
            if let Some(next) = paths.peek() {
                if next.starts_with(path) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "'{}' is a file in the bundle and a directory of '{}'",
                            path.display(),
                            next.display()
                        ),
                    ));
                }
            }
        }
        Ok(Self { files: bundle })
    }

    /// The paths of the files in the bundle, in order.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(PathBuf::as_path)
    }
}

fn not_found(path: &Path, what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: no {what} in the bundle", path.display()),
    )
}

impl Files for Bundle {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| not_found(path, "file"))
    }

    fn entries(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        let entries: BTreeSet<PathBuf> = self
            .files
            .keys()
            .filter_map(|path| {
                let rest = path.strip_prefix(dir).ok()?;
                let first = rest.components().next()?;
                Some(dir.join(first))
            })
            .collect();
        if entries.is_empty() {
            return Err(not_found(dir, "directory"));
        }
        Ok(entries.into_iter().collect())
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.files.keys().any(|p| p != path && p.starts_with(path))
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn bundle() -> Bundle {
        Bundle::new([
            ("cells/a/cel.yaml".to_string(), "name: A".to_string()),
            (
                "cells/a/reductions/r/2026-01-01.yaml".to_string(),
                "r".to_string(),
            ),
            ("cells/b/cel.yaml".to_string(), "name: B".to_string()),
            ("cells/ab/cel.yaml".to_string(), "name: AB".to_string()),
        ])
        .unwrap()
    }

    #[test]
    fn a_bundle_reads_its_files_and_lists_its_directories() {
        let b = bundle();
        assert_eq!(b.read(Path::new("cells/a/cel.yaml")).unwrap(), "name: A");
        assert!(b.is_dir(Path::new("cells/a")));
        assert!(b.is_dir(Path::new("cells")));
        assert!(!b.is_dir(Path::new("cells/a/cel.yaml")));
        assert!(b.is_file(Path::new("cells/a/cel.yaml")));
        assert!(!b.exists(Path::new("cells/c")));
        assert!(!b.is_dir(Path::new("cells/a/r")));
        // `cells/ab` is no entry of `cells/a`: paths compare by component.
        let mut entries = b.entries(Path::new("cells/a")).unwrap();
        entries.sort();
        assert_eq!(
            entries,
            [
                PathBuf::from("cells/a/cel.yaml"),
                PathBuf::from("cells/a/reductions")
            ]
        );
    }

    #[test]
    fn what_is_not_in_a_bundle_is_not_found() {
        let b = bundle();
        let e = b.read(Path::new("cells/c/cel.yaml")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        let e = b.entries(Path::new("cells/c")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn a_bundle_refuses_a_path_that_names_no_one_place() {
        for path in ["/cells/a", "cells/../a", "./cells/a", ""] {
            let e = Bundle::new([(path.to_string(), String::new())]).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{path}");
        }
        let twice = Bundle::new([
            ("a/b".to_string(), String::new()),
            ("a/b".to_string(), String::new()),
        ]);
        assert!(twice.is_err());
        let both = Bundle::new([
            ("a/b".to_string(), String::new()),
            ("a/b/c".to_string(), String::new()),
        ]);
        assert!(both.is_err());
    }
}
