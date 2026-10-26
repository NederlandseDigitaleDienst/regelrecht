//! Tier-2 extraction: the JavaScript/TypeScript/Vue frontends.
//!
//! Where `crate_graph`/`syn_pass` map the Rust workspace, this pass maps the
//! npm-workspace frontends — the Vue apps and their shared package — into the
//! same language-agnostic [`Model`]. It walks each app's `src/` tree, turns
//! every source file into a node (a `.vue` component, a `useXxx` composable, or
//! a plain module), nests them under directory grouping nodes, and turns ES
//! `import`s into edges.
//!
//! # Approach (decided, documented)
//!
//! Two routes were on the table (see the ticket and the README):
//!   1. a JS/TS parser **inside this Rust crate** (`swc_ecma_parser`, `oxc`);
//!   2. a **separate Node script** writing a partial model the Rust side reads.
//!
//! We take neither of the heavy options: a **self-contained regex scan of the
//! ES-module `import`/`export … from` statements**, staying inside the single
//! Rust binary. Rationale, mirroring the crate's existing `syn`-over-rustdoc
//! choice:
//!   - **No extra toolchain at generation time.** The explorer regenerates the
//!     model on-demand from the working tree; shelling out to Node would need
//!     `node_modules` installed and Node on `PATH` on every request, which the
//!     README's whole "self-contained stable-toolchain binary" argument rejects.
//!   - **The edges we need are import edges**, not a resolved type graph. A full
//!     AST parser (swc/oxc) is a large compile-time dependency for what amounts
//!     to reading the specifier strings — the same best-effort, structural
//!     stance `syn_pass` already documents (no name resolution).
//!   - **Vue SFCs** are not plain JS, so for `.vue` files we first cut out the
//!     `<script>` / `<script setup>` block(s); scanning the whole file would let
//!     template text (`… from "somewhere" …`) masquerade as an import.
//!
//! Like `syn_pass`, this is deliberately best-effort: a specifier inside a
//! comment or an unusual dynamic construction may be missed or over-counted, and
//! only imports that resolve to a file we actually emitted become edges.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde_json::Value;

use crate::crate_graph::rel;
use crate::model::{Edge, EdgeKind, Kind, Level, Node};

/// Source-file extensions we treat as JS/TS modules (besides `.vue`).
const JS_EXTS: &[&str] = &["js", "ts", "mjs", "cjs", "jsx", "tsx"];
/// Extensions tried, in order, when resolving an extension-less relative import.
const RESOLVE_EXTS: &[&str] = &["js", "ts", "vue", "mjs", "cjs", "jsx", "tsx"];
/// Directory names never descended into during the source walk.
const PRUNED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    ".git",
    "public",
    "coverage",
    "e2e",
    "e2e-smoke",
    "__snapshots__",
    ".vite",
];

/// A discovered npm-workspace frontend.
pub struct FrontendApp {
    /// Short label used in node ids: the app directory's path under the
    /// repository, without `packages/` and with `-` for `/`, e.g. `frontend`,
    /// `frontend-shared`, `poc-chronolexografie-frontend`.
    pub short: String,
    /// Stable node id, `app:<short>`.
    pub node_id: String,
    /// Absolute app directory.
    pub dir: PathBuf,
    /// The npm package name (from its `package.json`), used to turn a bare
    /// import of one app by another into a cross-app `depends-on` edge.
    pub package_name: String,
    /// Absolute path of the package's entry module (from `exports`/`module`/
    /// `main`, else `src/index.js`), so a bare package import can also resolve
    /// to a file-level `uses` edge.
    pub entry: Option<PathBuf>,
}

/// Reads a JSON file into a `serde_json::Value`, or `None` on any error.
fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// The absolute directories of the discovered frontends. Used both here and by
/// the `serve` cache to know which trees to watch for changes.
pub fn app_dirs(repo_root: &Path) -> Vec<PathBuf> {
    discover_apps(repo_root)
        .into_iter()
        .map(|a| a.dir)
        .collect()
}

/// Discovers the frontends from the root `package.json` `workspaces` list — the
/// single source of truth for which frontends exist, so this pass self-adjusts
/// when apps are added or removed. Entries may be literal dirs
/// (`frontend`) or a single trailing glob (`packages/*`).
fn discover_apps(repo_root: &Path) -> Vec<FrontendApp> {
    let Some(root_pkg) = read_json(&repo_root.join("package.json")) else {
        return Vec::new();
    };
    let Some(workspaces) = root_pkg.get("workspaces").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut member_dirs: Vec<PathBuf> = Vec::new();
    for entry in workspaces {
        let Some(pattern) = entry.as_str() else {
            continue;
        };
        if let Some(prefix) = pattern.strip_suffix("/*") {
            // One level of glob: every immediate subdirectory of `prefix`.
            let base = repo_root.join(prefix);
            if let Ok(read) = std::fs::read_dir(&base) {
                for e in read.flatten() {
                    if e.path().is_dir() {
                        member_dirs.push(e.path());
                    }
                }
            }
        } else if !pattern.contains('*') {
            member_dirs.push(repo_root.join(pattern));
        }
    }
    member_dirs.sort();
    member_dirs.dedup();

    let mut apps: Vec<FrontendApp> = Vec::new();
    for dir in member_dirs {
        let Some(pkg) = read_json(&dir.join("package.json")) else {
            continue;
        };
        // The directory's path under the repository, without `packages/` and
        // with `-` for `/`: `frontend`, `frontend-shared`, and for a frontend
        // inside a crate `poc-chronolexografie-frontend`. The name of the
        // directory alone is not unique: that one is called `frontend` too.
        let Some(short) = dir
            .strip_prefix(repo_root)
            .ok()
            .and_then(|rel| rel.to_str())
            .map(|rel| {
                rel.strip_prefix("packages/")
                    .unwrap_or(rel)
                    .replace('/', "-")
            })
        else {
            continue;
        };
        let short = short.as_str();
        let package_name = pkg
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(short)
            .to_string();
        apps.push(FrontendApp {
            short: short.to_string(),
            node_id: format!("app:{short}"),
            entry: resolve_entry(&dir, &pkg),
            dir,
            package_name,
        });
    }
    apps.sort_by(|a, b| a.short.cmp(&b.short));
    apps
}

/// Resolves a package's entry module to an absolute path, preferring the
/// `exports["."]` map, then `module`, then `main`, then `src/index.js`.
fn resolve_entry(dir: &Path, pkg: &Value) -> Option<PathBuf> {
    let from_exports = pkg
        .get("exports")
        .and_then(|e| e.get("."))
        .and_then(|dot| match dot {
            Value::String(s) => Some(s.clone()),
            // `{ ".": { "import": "./x.js", "default": "./x.js" } }`
            Value::Object(_) => dot
                .get("import")
                .or_else(|| dot.get("default"))
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        });
    let rel_entry = from_exports
        .or_else(|| {
            pkg.get("module")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| pkg.get("main").and_then(Value::as_str).map(str::to_string));

    let candidate = match rel_entry {
        Some(r) => dir.join(r.trim_start_matches("./")),
        None => dir.join("src/index.js"),
    };
    candidate.exists().then_some(lexical_normalize(&candidate))
}

/// Compiled regexes for import extraction. Built once per run.
struct ImportScan {
    /// `… from '<spec>'` (covers `import …`, `export … from`).
    from: Regex,
    /// Dynamic `import('<spec>')` and `import ( "<spec>" )`.
    dynamic: Regex,
    /// Bare side-effect `import '<spec>'` (no `from`).
    bare: Regex,
    /// One `<script …>…</script>` block body of a Vue SFC.
    script_block: Regex,
}

impl ImportScan {
    fn new() -> Option<Self> {
        Some(Self {
            from: Regex::new(r#"(?:^|[^.\w])from\s*['"]([^'"]+)['"]"#).ok()?,
            dynamic: Regex::new(r#"\bimport\s*\(\s*['"]([^'"]+)['"]"#).ok()?,
            bare: Regex::new(r#"(?m)^\s*import\s+['"]([^'"]+)['"]"#).ok()?,
            script_block: Regex::new(r"(?is)<script[^>]*>(.*?)</script>").ok()?,
        })
    }

    /// The JS relevant for import scanning: for a `.vue` file, the concatenated
    /// `<script>` block bodies; for everything else, the whole source.
    fn js_text(&self, is_vue: bool, source: &str) -> String {
        if !is_vue {
            return source.to_string();
        }
        let mut out = String::new();
        for cap in self.script_block.captures_iter(source) {
            if let Some(body) = cap.get(1) {
                out.push_str(body.as_str());
                out.push('\n');
            }
        }
        out
    }

    /// All import specifiers referenced by `js`, de-duplicated in first-seen
    /// order for deterministic output.
    fn specifiers(&self, js: &str) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        let mut push = |s: &str| {
            if !seen.iter().any(|x| x == s) {
                seen.push(s.to_string());
            }
        };
        for re in [&self.from, &self.dynamic, &self.bare] {
            for cap in re.captures_iter(js) {
                if let Some(m) = cap.get(1) {
                    push(m.as_str());
                }
            }
        }
        seen
    }
}

/// How a source file is classified into an id prefix, kind and level.
struct FileClass {
    prefix: &'static str,
    kind: Kind,
    lang: &'static str,
}

/// Classifies a file purely by its path (never its contents), so a node's id is
/// stable: a `.vue` is a component, a `useXxx.{js,ts}` is a composable, any
/// other JS/TS file is a module.
fn classify(path: &Path) -> Option<FileClass> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    let stem = path.file_stem().and_then(|s| s.to_str())?;

    if ext == "vue" {
        return Some(FileClass {
            prefix: "component",
            kind: Kind::Component,
            lang: "vue",
        });
    }
    if !JS_EXTS.contains(&ext) {
        return None;
    }
    let lang = if ext == "ts" || ext == "tsx" {
        "ts"
    } else {
        "js"
    };
    if is_composable_name(stem) {
        return Some(FileClass {
            prefix: "composable",
            kind: Kind::Composable,
            lang,
        });
    }
    Some(FileClass {
        prefix: "module",
        kind: Kind::Module,
        lang,
    })
}

/// A composable follows Vue's `useXxx` convention: `use` followed by an
/// uppercase letter (so `useAuth` matches but `user`/`utils` do not).
fn is_composable_name(stem: &str) -> bool {
    stem.strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_uppercase())
}

/// Skips test, type-declaration and config files — the JS analogue of
/// `syn_pass` skipping `#[cfg(test)]` code and `build.rs`.
fn is_excluded_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with(".d.ts")
        || name.ends_with(".config.js")
        || name.ends_with(".config.ts")
        || name.ends_with(".config.mjs")
        || name.ends_with(".config.cjs")
}

/// Lexically normalizes a path (resolving `.`/`..` without touching the
/// filesystem), so import-resolution keys match the walked file paths
/// deterministically regardless of symlinks.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A file turned into a node, plus the import specifiers found in it, pending
/// resolution once every file's node id is known.
struct PendingFile {
    node_id: String,
    app_node_id: String,
    abs: PathBuf,
    specifiers: Vec<String>,
}

/// Extracts all frontend apps into `nodes`/`edges`. The entry point called from
/// `build_model` after the Rust tiers.
pub fn extract(repo_root: &Path, nodes: &mut Vec<Node>, edges: &mut Vec<Edge>) {
    let apps = discover_apps(repo_root);
    if apps.is_empty() {
        return;
    }
    let Some(scan) = ImportScan::new() else {
        return;
    };

    // abs file path -> node id, for resolving relative imports to file nodes.
    let mut file_by_abs: HashMap<PathBuf, String> = HashMap::new();
    // Ensure each directory grouping node is emitted once (id -> node index).
    let mut seen_dirs: HashMap<String, usize> = HashMap::new();
    let mut pending: Vec<PendingFile> = Vec::new();

    for app in &apps {
        nodes.push(Node {
            id: app.node_id.clone(),
            level: Level::Container,
            kind: Kind::App,
            lang: "js".to_string(),
            name: app.short.clone(),
            path: rel(repo_root, &app.dir),
            parent: None,
            doc: None,
        });

        for file in app_source_files(&app.dir) {
            let Some(class) = classify(&file) else {
                continue;
            };
            let rel_segs = match segments_from(&app.dir, &file) {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };

            // Directory chain: every segment except the filename becomes a
            // `dir:` node; the file hangs off the deepest one (or the app node).
            let dir_segs = &rel_segs[..rel_segs.len() - 1];
            let parent_id = ensure_dir_chain(app, repo_root, dir_segs, nodes, &mut seen_dirs);

            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let mut id_segs: Vec<&str> = dir_segs.iter().map(String::as_str).collect();
            id_segs.push(&stem);
            let node_id = format!("{}:{}::{}", class.prefix, app.short, id_segs.join("::"));

            nodes.push(Node {
                id: node_id.clone(),
                level: Level::Component,
                kind: class.kind,
                lang: class.lang.to_string(),
                name: file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&stem)
                    .to_string(),
                path: rel(repo_root, &file),
                parent: Some(parent_id),
                doc: None,
            });

            let abs = lexical_normalize(&file);
            let js = scan.js_text(class.lang == "vue", &read_or_empty(&file));
            let specifiers = scan.specifiers(&js);
            file_by_abs.insert(abs.clone(), node_id.clone());
            pending.push(PendingFile {
                node_id,
                app_node_id: app.node_id.clone(),
                abs,
                specifiers,
            });
        }
    }

    resolve_imports(&apps, &file_by_abs, &pending, edges);
}

/// Reads a file to a string, or an empty string on error (a file that can't be
/// read simply yields no imports — best-effort, like `syn_pass`).
fn read_or_empty(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// The source files of one app: everything under `src/` plus any top-level entry
/// module (`main`/`index`), minus test/config/declaration files. Sorted for
/// determinism.
fn app_source_files(app_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    // The app's `src/` tree.
    let src = app_dir.join("src");
    if src.is_dir() {
        collect_source_files(&src, &mut files);
    }

    // Top-level entry modules that live outside `src/` (a `main.js` next to
    // `index.html`), so an app whose bootstrap sits at its root is still covered.
    for name in ["main", "index"] {
        for ext in JS_EXTS {
            let candidate = app_dir.join(format!("{name}.{ext}"));
            if candidate.is_file() && !is_excluded_file(&candidate) {
                files.push(candidate);
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

/// Recursively collects candidate source files under `dir`, pruning build and
/// vendor directories and skipping test/config/declaration files.
fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in walkdir::WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !is_pruned_dir(e))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || is_excluded_file(path) {
            continue;
        }
        let is_source = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext == "vue" || JS_EXTS.contains(&ext));
        if is_source {
            out.push(path.to_path_buf());
        }
    }
}

fn is_pruned_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|n| PRUNED_DIRS.contains(&n))
}

/// Path components of `file` relative to `app_dir` (e.g. `["src", "components",
/// "LawTable.vue"]`).
fn segments_from(app_dir: &Path, file: &Path) -> Option<Vec<String>> {
    Some(
        file.strip_prefix(app_dir)
            .ok()?
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
            .collect(),
    )
}

/// Emits a `dir:` node for `segs` and any missing ancestor directories,
/// returning the id of the deepest (the file's parent). Empty `segs` means the
/// file sits at the app root, so the app node is the parent. Idempotent via
/// `seen`.
fn ensure_dir_chain(
    app: &FrontendApp,
    repo_root: &Path,
    segs: &[String],
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<String, usize>,
) -> String {
    let mut parent = app.node_id.clone();
    let mut acc: Vec<String> = Vec::new();
    let mut acc_path = app.dir.clone();
    for seg in segs {
        acc.push(seg.clone());
        acc_path.push(seg);
        let id = format!("dir:{}::{}", app.short, acc.join("::"));
        if !seen.contains_key(&id) {
            let idx = nodes.len();
            nodes.push(Node {
                id: id.clone(),
                level: Level::Component,
                kind: Kind::Dir,
                lang: "js".to_string(),
                name: seg.clone(),
                path: rel(repo_root, &acc_path),
                parent: Some(parent.clone()),
                doc: None,
            });
            seen.insert(id.clone(), idx);
        }
        parent = id;
    }
    parent
}

/// Turns each pending file's import specifiers into edges: a relative import
/// resolved to a known file node is a `uses` edge; a bare import of another
/// workspace app is a cross-app `depends-on` edge (plus a `uses` edge to that
/// app's entry module when the import targets the package root).
fn resolve_imports(
    apps: &[FrontendApp],
    file_by_abs: &HashMap<PathBuf, String>,
    pending: &[PendingFile],
    edges: &mut Vec<Edge>,
) {
    for file in pending {
        let Some(dir) = file.abs.parent() else {
            continue;
        };
        for spec in &file.specifiers {
            if spec.starts_with('.') {
                if let Some(target) = resolve_relative(dir, spec, file_by_abs) {
                    if target != file.node_id {
                        edges.push(Edge {
                            from: file.node_id.clone(),
                            to: target,
                            kind: EdgeKind::Uses,
                        });
                    }
                }
            } else {
                resolve_bare(apps, file, spec, file_by_abs, edges);
            }
        }
    }
}

/// Resolves a relative specifier to a file node id, trying the specifier as-is,
/// then with each source extension, then as a directory `index.<ext>`.
fn resolve_relative(
    from_dir: &Path,
    spec: &str,
    file_by_abs: &HashMap<PathBuf, String>,
) -> Option<String> {
    let base = lexical_normalize(&from_dir.join(spec));

    if let Some(id) = file_by_abs.get(&base) {
        return Some(id.clone());
    }
    for ext in RESOLVE_EXTS {
        let with_ext = base.with_extension(ext);
        if let Some(id) = file_by_abs.get(&with_ext) {
            return Some(id.clone());
        }
    }
    for ext in RESOLVE_EXTS {
        let index = base.join(format!("index.{ext}"));
        if let Some(id) = file_by_abs.get(&index) {
            return Some(id.clone());
        }
    }
    None
}

/// Resolves a bare specifier that names another workspace app (by package name)
/// into a cross-app `depends-on` edge, and — when the whole package (not a
/// subpath) is imported — a `uses` edge to that app's entry module.
fn resolve_bare(
    apps: &[FrontendApp],
    file: &PendingFile,
    spec: &str,
    file_by_abs: &HashMap<PathBuf, String>,
    edges: &mut Vec<Edge>,
) {
    for app in apps {
        let whole = spec == app.package_name;
        let subpath = spec.starts_with(&format!("{}/", app.package_name));
        if !whole && !subpath {
            continue;
        }
        if app.node_id != file.app_node_id {
            edges.push(Edge {
                from: file.app_node_id.clone(),
                to: app.node_id.clone(),
                kind: EdgeKind::DependsOn,
            });
        }
        if whole {
            if let Some(entry) = &app.entry {
                if let Some(target) = file_by_abs.get(entry) {
                    if target != &file.node_id {
                        edges.push(Edge {
                            from: file.node_id.clone(),
                            to: target.clone(),
                            kind: EdgeKind::Uses,
                        });
                    }
                }
            }
        }
        return;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_path_only() {
        assert_eq!(
            classify(Path::new("a/Foo.vue")).unwrap().prefix,
            "component"
        );
        assert_eq!(
            classify(Path::new("a/useAuth.js")).unwrap().prefix,
            "composable"
        );
        assert_eq!(classify(Path::new("a/router.js")).unwrap().prefix, "module");
        assert_eq!(classify(Path::new("a/useAuth.ts")).unwrap().lang, "ts");
        assert!(classify(Path::new("a/styles.css")).is_none());
    }

    #[test]
    fn composable_needs_uppercase_after_use() {
        assert!(is_composable_name("useAuth"));
        assert!(is_composable_name("useColorScheme"));
        assert!(!is_composable_name("user"));
        assert!(!is_composable_name("utils"));
        assert!(!is_composable_name("use"));
    }

    #[test]
    fn excludes_test_config_and_decl_files() {
        assert!(is_excluded_file(Path::new(
            "a/LibraryView.docReview.test.js"
        )));
        assert!(is_excluded_file(Path::new("a/thing.spec.ts")));
        assert!(is_excluded_file(Path::new("a/vite.config.js")));
        assert!(is_excluded_file(Path::new("a/shims.d.ts")));
        assert!(!is_excluded_file(Path::new("a/router.js")));
    }

    #[test]
    fn scans_imports_including_vue_script_only() {
        let scan = ImportScan::new().unwrap();
        let vue = r#"
<template><p>copied from "the template"</p></template>
<script setup>
import { ref } from 'vue';
import Foo from './Foo.vue';
export { bar } from '../lib/bar.js';
const lazy = () => import('./Lazy.vue');
</script>
"#;
        let js = scan.js_text(true, vue);
        let specs = scan.specifiers(&js);
        assert!(specs.contains(&"vue".to_string()));
        assert!(specs.contains(&"./Foo.vue".to_string()));
        assert!(specs.contains(&"../lib/bar.js".to_string()));
        assert!(specs.contains(&"./Lazy.vue".to_string()));
        // The template's `from "the template"` must NOT be picked up.
        assert!(!specs.iter().any(|s| s.contains("template")));
    }

    #[test]
    fn side_effect_import_is_scanned() {
        let scan = ImportScan::new().unwrap();
        let specs = scan.specifiers("import './styles.css';\nimport x from 'y';");
        assert!(specs.contains(&"./styles.css".to_string()));
        assert!(specs.contains(&"y".to_string()));
    }

    #[test]
    fn lexical_normalize_resolves_dotdot() {
        assert_eq!(
            lexical_normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }
}
