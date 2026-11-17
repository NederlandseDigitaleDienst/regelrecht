//! Tier-1b extraction: source-level structure via `syn`.
//!
//! For every workspace crate we walk `src/**.rs`, derive each file's module
//! path from its location, and parse it with `syn` (pinned stable — no nightly
//! rustdoc-JSON needed). We emit module / struct / enum / trait / method / fn
//! nodes (with the first line of their doc-comment) plus `impl` and `uses`
//! edges.
//!
//! This is deliberately a *structural* pass, not a full name-resolution pass:
//! syn sees one file at a time with no type inference or macro expansion. To
//! still capture how the parts use each other — the whole point of the explorer
//! — the pass takes the "middle road" the README describes:
//!
//! 1. It builds a **workspace-wide symbol table** (crate → declared type leaf →
//!    node id) across *all* deep-parsed crates, so a reference can resolve to a
//!    type in another crate, not just the file's own.
//! 2. Per file it reads the `use` statements into a **scope map** (in-scope name
//!    → target crate + leaf). That turns `use`-aliases and multi-segment paths
//!    (`use regelrecht_law_model::foo::Bar as Baz`) into concrete targets
//!    without a type checker.
//! 3. A reference resolves to a node only when the crate is known *and* the leaf
//!    name is unambiguous inside that crate. Anything that stays ambiguous, or
//!    whose crate can't be identified (an external crate, a glob import, a
//!    macro-introduced name), is **dropped rather than guessed** — a false edge
//!    is worse than a missing one.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;

use syn::visit::Visit;

use crate::crate_graph::{CrateInfo, rel};
use crate::model::{Edge, EdgeKind, Kind, Level, Node};

/// A reference resolved to a concrete workspace target: a crate short-name plus
/// the leaf identifier of the referenced type/trait. The final leaf → node-id
/// lookup happens in the second pass against the workspace symbol table.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Target {
    krate: String,
    leaf: String,
}

/// What an in-scope name (introduced by a `use`) points at.
#[derive(Clone, PartialEq, Eq, Debug)]
enum UseTarget {
    /// A known workspace crate + leaf (resolve against the symbol table).
    Workspace(Target),
    /// Explicitly imported from a non-workspace crate (e.g. `std`, `serde`).
    /// Recorded so a bare reference to this name is *not* leaf-matched against
    /// the local crate — that would be a false edge.
    External,
}

/// A `uses` reference to resolve later: the owner node id and its target.
struct PendingRef {
    owner: String,
    target: Target,
}

/// A pending `impl Type for Trait` edge, both sides still leaf-level.
struct PendingImpl {
    ty: Target,
    tr: Target,
}

/// Everything harvested from a single crate before edge resolution.
struct CrateItems {
    short: String,
    nodes: Vec<Node>,
    trait_impls: Vec<PendingImpl>,
    type_refs: Vec<PendingRef>,
    /// Type leaf name -> node id, for edge resolution. When a name is ambiguous
    /// (defined twice in the crate) it is dropped to avoid guessing wrong.
    type_by_name: HashMap<String, Option<String>>,
}

/// Runs the deep source pass over every `krate` in `crates`, appending nodes and
/// edges. Edges are resolved in a second pass so a reference can point at a type
/// declared in *another* crate — see the module docs.
pub fn extract(
    repo_root: &Path,
    crates: &[&CrateInfo],
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    // Map every extern-crate ident (`regelrecht_law_model`) to its short name
    // (`law-model`) so `use` paths can be attributed to a crate.
    let crate_by_ident: HashMap<String, String> = crates
        .iter()
        .map(|c| (c.ident.clone(), c.short.clone()))
        .collect();

    // Pass 1: collect nodes + pending refs for each crate.
    let mut per_crate: Vec<CrateItems> = Vec::new();
    for krate in crates {
        per_crate.push(collect_crate(repo_root, krate, &crate_by_ident));
    }

    // Build the workspace symbol table: crate short -> (leaf -> node id).
    let symbols: HashMap<String, HashMap<String, Option<String>>> = per_crate
        .iter()
        .map(|c| (c.short.clone(), c.type_by_name.clone()))
        .collect();

    // Pass 2: resolve edges against the full symbol table.
    for items in &mut per_crate {
        for r in &items.type_refs {
            if let Some(to) = lookup(&symbols, &r.target)
                && to != r.owner
            {
                edges.push(Edge {
                    from: r.owner.clone(),
                    to,
                    kind: EdgeKind::Uses,
                });
            }
        }
        for im in &items.trait_impls {
            if let (Some(from), Some(to)) = (lookup(&symbols, &im.ty), lookup(&symbols, &im.tr)) {
                edges.push(Edge {
                    from,
                    to,
                    kind: EdgeKind::Impl,
                });
            }
        }
        nodes.append(&mut items.nodes);
    }
}

/// Resolves a [`Target`] to a node id: the crate must be known and the leaf must
/// name exactly one type in it. An ambiguous leaf (`Some(None)`) or a missing
/// one yields `None` — deliberately dropping the reference rather than guessing.
fn lookup(
    symbols: &HashMap<String, HashMap<String, Option<String>>>,
    target: &Target,
) -> Option<String> {
    symbols.get(&target.krate)?.get(&target.leaf)?.clone()
}

/// Extracts nodes and pending refs for one crate (pass 1). No edges are emitted
/// here — resolution needs the whole-workspace symbol table.
fn collect_crate(
    repo_root: &Path,
    krate: &CrateInfo,
    crate_by_ident: &HashMap<String, String>,
) -> CrateItems {
    let mut items = CrateItems {
        short: krate.short.clone(),
        nodes: Vec::new(),
        trait_impls: Vec::new(),
        type_refs: Vec::new(),
        type_by_name: HashMap::new(),
    };

    let src = krate.dir.join("src");
    if !src.is_dir() {
        return items;
    }

    // Ensure every ancestor module node exists exactly once (id → node index).
    let mut seen_modules: HashMap<String, usize> = HashMap::new();

    let mut files: Vec<_> = walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(walkdir::DirEntry::into_path)
        .collect();
    files.sort();

    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(ast) = syn::parse_file(&source) else {
            eprintln!("arch-extract: skipping unparseable file {}", file.display());
            continue;
        };

        let Some(base) = base_module(&src, file) else {
            continue;
        };

        // The file's `use` statements, resolved once for the whole file (nested
        // `mod` blocks included) into an in-scope-name → target map.
        let mut use_scope: HashMap<String, UseTarget> = HashMap::new();
        collect_file_uses(&ast.items, &krate.short, crate_by_ident, &mut use_scope);

        // The container the file's top-level items hang from, and the module
        // path segments that prefix their ids.
        let (parent_id, mod_path) = match &base {
            FileRole::CrateRoot => (krate.node_id.clone(), Vec::new()),
            FileRole::Binary(name) => {
                let id = format!("bin:{}::{name}", krate.short);
                items.nodes.push(Node {
                    id: id.clone(),
                    level: Level::Container,
                    kind: Kind::Binary,
                    lang: "rust".to_string(),
                    name: name.clone(),
                    path: rel(repo_root, file),
                    parent: Some(krate.node_id.clone()),
                    doc: first_doc(&ast.attrs),
                });
                (id, Vec::new())
            }
            FileRole::Module(segs) => {
                let id = ensure_module_chain(
                    krate,
                    repo_root,
                    file,
                    segs,
                    first_doc(&ast.attrs),
                    &mut items.nodes,
                    &mut seen_modules,
                );
                (id, segs.clone())
            }
        };

        let mut visitor = ItemVisitor {
            krate,
            repo_root,
            file,
            mod_path,
            parent_id,
            crate_by_ident,
            use_scope: &use_scope,
            items: &mut items,
            seen_modules: &mut seen_modules,
        };
        for item in &ast.items {
            visitor.visit_item(item);
        }
    }

    // Reconcile method parents: a method's parent id is built from the impl's
    // own module prefix, but the type may live in another file of the crate.
    // Repoint to the real type node when its leaf name resolves unambiguously.
    let type_ids: std::collections::HashSet<String> = items
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, Kind::Struct | Kind::Enum | Kind::Trait))
        .map(|n| n.id.clone())
        .collect();
    for n in &mut items.nodes {
        if n.kind != Kind::Method {
            continue;
        }
        let Some(parent) = &n.parent else { continue };
        if type_ids.contains(parent) {
            continue;
        }
        if let Some(leaf) = parent.rsplit("::").next()
            && let Some(Some(real)) = items.type_by_name.get(leaf)
        {
            n.parent = Some(real.clone());
        }
    }

    items
}

/// What a source file represents in the module tree.
enum FileRole {
    CrateRoot,
    Binary(String),
    Module(Vec<String>),
}

/// Maps a file path (relative to the crate `src/`) to its role/module path.
fn base_module(src: &Path, file: &Path) -> Option<FileRole> {
    let relp = file.strip_prefix(src).ok()?;
    let comps: Vec<String> = relp
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    let (dirs, filename) = comps.split_at(comps.len().saturating_sub(1));
    let filename = filename.first()?.as_str();
    let stem = filename.strip_suffix(".rs")?;

    // `src/bin/<name>.rs` is a separate binary target.
    if dirs.first().map(String::as_str) == Some("bin") && dirs.len() == 1 {
        return Some(FileRole::Binary(stem.to_string()));
    }

    let mut segs: Vec<String> = dirs.to_vec();
    match stem {
        // Crate/module roots.
        "lib" | "main" if dirs.is_empty() => return Some(FileRole::CrateRoot),
        "mod" => { /* `foo/mod.rs` → module `foo` (dirs already hold it). */ }
        other => segs.push(other.to_string()),
    }
    if segs.is_empty() {
        Some(FileRole::CrateRoot)
    } else {
        Some(FileRole::Module(segs))
    }
}

/// Emits a module node for `segs` and any missing ancestors, returning the id
/// of the deepest one. Idempotent via `seen` (id → index into `nodes`). When a
/// module is first seen as an ancestor (empty path) and later visited as the
/// file that actually defines it, its path/doc are backfilled — so the node
/// points at the module's own source, not wherever it was first referenced.
fn ensure_module_chain(
    krate: &CrateInfo,
    repo_root: &Path,
    file: &Path,
    segs: &[String],
    doc: Option<String>,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<String, usize>,
) -> String {
    let mut parent = krate.node_id.clone();
    let mut acc: Vec<String> = Vec::new();
    let last = segs.len().saturating_sub(1);
    for (i, seg) in segs.iter().enumerate() {
        acc.push(seg.clone());
        let id = format!("mod:{}::{}", krate.short, acc.join("::"));
        // Only the deepest segment of this call is being *defined* here; the
        // ancestors are just containment and get their real path when visited.
        let defining = i == last;
        let this_path = if defining {
            rel(repo_root, file)
        } else {
            String::new()
        };
        match seen.get(&id) {
            Some(&idx) => {
                if defining && nodes[idx].path.is_empty() {
                    nodes[idx].path = this_path;
                    nodes[idx].doc = doc.clone();
                }
            }
            None => {
                let idx = nodes.len();
                nodes.push(Node {
                    id: id.clone(),
                    level: Level::Component,
                    kind: Kind::Module,
                    lang: "rust".to_string(),
                    name: seg.clone(),
                    path: this_path,
                    parent: Some(parent.clone()),
                    doc: if defining { doc.clone() } else { None },
                });
                seen.insert(id.clone(), idx);
            }
        }
        parent = id;
    }
    parent
}

/// Walks the items of one file, tracking the current module path so nested
/// `mod x { .. }` blocks and their contents get correct ids and parents.
struct ItemVisitor<'a> {
    krate: &'a CrateInfo,
    repo_root: &'a Path,
    file: &'a Path,
    mod_path: Vec<String>,
    parent_id: String,
    crate_by_ident: &'a HashMap<String, String>,
    use_scope: &'a HashMap<String, UseTarget>,
    items: &'a mut CrateItems,
    seen_modules: &'a mut HashMap<String, usize>,
}

impl ItemVisitor<'_> {
    /// `crate::a::b` label for the current module, used to build ids.
    fn path_prefix(&self) -> String {
        if self.mod_path.is_empty() {
            self.krate.short.clone()
        } else {
            format!("{}::{}", self.krate.short, self.mod_path.join("::"))
        }
    }

    fn record_type(&mut self, name: &str, id: &str) {
        self.items
            .type_by_name
            .entry(name.to_string())
            .and_modify(|slot| *slot = None) // ambiguous → don't resolve edges to it
            .or_insert_with(|| Some(id.to_string()));
    }

    /// Resolves a written type path (its segments) to a workspace [`Target`], or
    /// `None` when it cannot be attributed to a known crate without guessing.
    fn resolve(&self, segs: &[String]) -> Option<Target> {
        resolve_path(segs, &self.krate.short, self.use_scope, self.crate_by_ident)
    }

    /// Records every type referenced by `ty` (recursing through generics,
    /// references, tuples, `dyn`/`impl Trait` bounds) as a pending `uses` edge
    /// from `owner`.
    fn push_uses(&mut self, owner: &str, ty: &syn::Type) {
        let mut paths = Vec::new();
        collect_type_paths(ty, &mut paths);
        for segs in paths {
            if let Some(target) = self.resolve(&segs) {
                self.items.type_refs.push(PendingRef {
                    owner: owner.to_string(),
                    target,
                });
            }
        }
    }

    /// Records the parameter and return types of a signature as `uses` edges
    /// from `owner` (a type node for methods, the fn node for free functions).
    fn push_sig_uses(&mut self, owner: &str, sig: &syn::Signature) {
        for input in &sig.inputs {
            if let syn::FnArg::Typed(pt) = input {
                self.push_uses(owner, &pt.ty);
            }
        }
        if let syn::ReturnType::Type(_, ty) = &sig.output {
            self.push_uses(owner, ty);
        }
    }
}

impl<'ast> Visit<'ast> for ItemVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if is_test_gated(&node.attrs) {
            return;
        }
        // A bodyless `mod foo;` is only a declaration; the file `foo.rs` /
        // `foo/mod.rs` is walked separately and defines the node with its real
        // path. Skip the declaration so the node points at the definition.
        let Some((_, items)) = &node.content else {
            return;
        };

        let name = node.ident.to_string();
        let mut child_path = self.mod_path.clone();
        child_path.push(name.clone());

        let id = ensure_module_chain(
            self.krate,
            self.repo_root,
            self.file,
            &child_path,
            first_doc(&node.attrs),
            &mut self.items.nodes,
            self.seen_modules,
        );

        let saved_path = std::mem::replace(&mut self.mod_path, child_path);
        let saved_parent = std::mem::replace(&mut self.parent_id, id);
        for item in items {
            self.visit_item(item);
        }
        self.mod_path = saved_path;
        self.parent_id = saved_parent;
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id: id.clone(),
            level: Level::Component,
            kind: Kind::Struct,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // `uses`: each field's referenced types, resolved to their crate later.
        for field in &node.fields {
            self.push_uses(&id, &field.ty);
        }
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id: id.clone(),
            level: Level::Component,
            kind: Kind::Enum,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // `uses`: the types carried by each variant's fields.
        for variant in &node.variants {
            for field in &variant.fields {
                self.push_uses(&id, &field.ty);
            }
        }
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id: id.clone(),
            level: Level::Component,
            kind: Kind::Trait,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // Trait methods with a default or a signature become code-level nodes,
        // and their signatures contribute `uses` edges from the trait.
        for item in &node.items {
            if let syn::TraitItem::Fn(f) = item {
                let m = f.sig.ident.to_string();
                self.items.nodes.push(Node {
                    id: format!("fn:{}::{m}", trim_type_prefix(&id)),
                    level: Level::Code,
                    kind: Kind::Method,
                    lang: "rust".to_string(),
                    name: m,
                    path: rel(self.repo_root, self.file),
                    parent: Some(id.clone()),
                    doc: first_doc(&f.attrs),
                });
                self.push_sig_uses(&id, &f.sig);
            }
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.sig.ident.to_string();
        let fn_id = format!("fn:{}::{name}", self.path_prefix());
        self.items.nodes.push(Node {
            id: fn_id.clone(),
            level: Level::Code,
            kind: Kind::Fn,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // `uses`: a free function's signature types, attributed to the fn node.
        self.push_sig_uses(&fn_id, &node.sig);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if is_test_gated(&node.attrs) {
            return;
        }
        // The type this impl is for. Resolve its path (usually the same crate)
        // so methods and the `impl` edge attach to the right node.
        let mut self_paths = Vec::new();
        collect_type_paths(&node.self_ty, &mut self_paths);
        let Some(self_segs) = self_paths.first() else {
            return;
        };
        let Some(ty_leaf) = self_segs.last().cloned() else {
            return;
        };
        let self_target = self.resolve(self_segs);

        // `impl Trait for Type` → pending impl edge (type → trait). The trait
        // may live in another crate; resolution handles that.
        if let Some((path, _)) = &node.trait_ {
            let tr_segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            if let (Some(ty), Some(tr)) = (self_target.clone(), self.resolve(&tr_segs)) {
                self.items.trait_impls.push(PendingImpl { ty, tr });
            }
        }
        // Methods hang off the type node id (built from the impl's own type
        // label so it is stable even if the type lives in another file), and
        // their signatures contribute `uses` edges from that type.
        let type_id = format!("type:{}::{ty_leaf}", self.path_prefix());
        for item in &node.items {
            if let syn::ImplItem::Fn(f) = item {
                let m = f.sig.ident.to_string();
                self.items.nodes.push(Node {
                    id: format!("fn:{}::{ty_leaf}::{m}", self.path_prefix()),
                    level: Level::Code,
                    kind: Kind::Method,
                    lang: "rust".to_string(),
                    name: m,
                    path: rel(self.repo_root, self.file),
                    parent: Some(type_id.clone()),
                    doc: first_doc(&f.attrs),
                });
                self.push_sig_uses(&type_id, &f.sig);
            }
        }
    }
}

/// Resolves a written type path to a workspace [`Target`], or `None` when the
/// crate can't be identified without guessing. This is where `use`-aliases,
/// multi-segment paths and cross-crate references are turned into a concrete
/// (crate, leaf) pair:
///
/// - a single-segment name uses the file's `use` scope, falling back to a
///   same-crate leaf (a bare `Foo` is either local or a prelude type — the
///   later symbol-table lookup drops it if it isn't a declared local type);
/// - a name explicitly imported from a non-workspace crate is dropped;
/// - a multi-segment path is attributed via its head: `crate`/`self`/`super`
///   and a known crate ident resolve; an unknown head (`std::…`, `serde::…`) is
///   dropped rather than leaf-matched, which is what keeps external paths from
///   becoming false local edges.
fn resolve_path(
    segs: &[String],
    current_short: &str,
    use_scope: &HashMap<String, UseTarget>,
    crate_by_ident: &HashMap<String, String>,
) -> Option<Target> {
    let head = segs.first()?;
    let leaf = segs.last()?.clone();

    if segs.len() == 1 {
        return match use_scope.get(head) {
            Some(UseTarget::Workspace(t)) => Some(t.clone()),
            Some(UseTarget::External) => None,
            None => Some(Target {
                krate: current_short.to_string(),
                leaf,
            }),
        };
    }

    match head.as_str() {
        "crate" | "self" | "super" | "Self" => Some(Target {
            krate: current_short.to_string(),
            leaf,
        }),
        _ => {
            if let Some(short) = crate_by_ident.get(head) {
                Some(Target {
                    krate: short.clone(),
                    leaf,
                })
            } else if let Some(UseTarget::Workspace(t)) = use_scope.get(head) {
                // `head` is a module/type brought into scope by a `use`; keep its
                // crate but take this path's own leaf.
                Some(Target {
                    krate: t.krate.clone(),
                    leaf,
                })
            } else {
                None
            }
        }
    }
}

/// Reads all `use` statements of a file (recursing into inline `mod` blocks)
/// into an in-scope-name → [`UseTarget`] map. When the same name is imported
/// twice with different targets (e.g. from two nested modules) it is downgraded
/// to [`UseTarget::External`] so it is never resolved — again preferring a
/// missing edge over a wrong one.
fn collect_file_uses(
    items: &[syn::Item],
    current_short: &str,
    crate_by_ident: &HashMap<String, String>,
    scope: &mut HashMap<String, UseTarget>,
) {
    for item in items {
        match item {
            syn::Item::Use(u) => {
                flatten_use(
                    &u.tree,
                    &mut Vec::new(),
                    current_short,
                    crate_by_ident,
                    scope,
                );
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_file_uses(inner, current_short, crate_by_ident, scope);
                }
            }
            _ => {}
        }
    }
}

/// Flattens one `use` tree into scope entries, carrying the path prefix down
/// through groups. Globs are skipped (their names are unknown).
fn flatten_use(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    current_short: &str,
    crate_by_ident: &HashMap<String, String>,
    scope: &mut HashMap<String, UseTarget>,
) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten_use(&p.tree, prefix, current_short, crate_by_ident, scope);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let name = n.ident.to_string();
            let mut full = prefix.clone();
            full.push(name.clone());
            insert_use(scope, name, &full, current_short, crate_by_ident);
        }
        syn::UseTree::Rename(r) => {
            let mut full = prefix.clone();
            full.push(r.ident.to_string());
            insert_use(
                scope,
                r.rename.to_string(),
                &full,
                current_short,
                crate_by_ident,
            );
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                flatten_use(t, prefix, current_short, crate_by_ident, scope);
            }
        }
        syn::UseTree::Glob(_) => { /* names unknown — cannot resolve safely */ }
    }
}

/// Turns one flattened `use` path into a scope entry under `alias`.
fn insert_use(
    scope: &mut HashMap<String, UseTarget>,
    alias: String,
    full: &[String],
    current_short: &str,
    crate_by_ident: &HashMap<String, String>,
) {
    let (Some(head), Some(leaf)) = (full.first(), full.last()) else {
        return;
    };
    let target = match head.as_str() {
        "crate" | "self" | "super" => UseTarget::Workspace(Target {
            krate: current_short.to_string(),
            leaf: leaf.clone(),
        }),
        _ => match crate_by_ident.get(head) {
            Some(short) => UseTarget::Workspace(Target {
                krate: short.clone(),
                leaf: leaf.clone(),
            }),
            None => UseTarget::External,
        },
    };
    match scope.entry(alias) {
        Entry::Occupied(mut e) => {
            if *e.get() != target {
                e.insert(UseTarget::External);
            }
        }
        Entry::Vacant(v) => {
            v.insert(target);
        }
    }
}

/// `type:a::b::Foo` → `a::b::Foo`, for composing method ids.
fn trim_type_prefix(type_id: &str) -> String {
    type_id.strip_prefix("type:").unwrap_or(type_id).to_string()
}

/// Collects the paths of every type referenced by `ty`, recursing through
/// generic arguments, references, tuples and `dyn`/`impl Trait` bounds (so
/// `Option<Vec<Foo>>` yields `[Option]`, `[Vec]`, `[Foo]` and `Box<dyn Bar>`
/// yields `[Box]`, `[Bar]`). Each entry is a full segment list so the resolver
/// can inspect the leading crate segment; best-effort and intentionally shallow.
fn collect_type_paths(ty: &syn::Type, out: &mut Vec<Vec<String>>) {
    match ty {
        syn::Type::Path(tp) => {
            let segs: Vec<String> = tp
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if !segs.is_empty() {
                out.push(segs);
            }
            if let Some(last) = tp.path.segments.last()
                && let syn::PathArguments::AngleBracketed(args) = &last.arguments
            {
                for arg in &args.args {
                    if let syn::GenericArgument::Type(inner) = arg {
                        collect_type_paths(inner, out);
                    }
                }
            }
        }
        syn::Type::Reference(r) => collect_type_paths(&r.elem, out),
        syn::Type::Slice(s) => collect_type_paths(&s.elem, out),
        syn::Type::Array(a) => collect_type_paths(&a.elem, out),
        syn::Type::Ptr(p) => collect_type_paths(&p.elem, out),
        syn::Type::Paren(p) => collect_type_paths(&p.elem, out),
        syn::Type::Group(g) => collect_type_paths(&g.elem, out),
        syn::Type::Tuple(t) => {
            for e in &t.elems {
                collect_type_paths(e, out);
            }
        }
        syn::Type::TraitObject(to) => collect_bound_paths(&to.bounds, out),
        syn::Type::ImplTrait(it) => collect_bound_paths(&it.bounds, out),
        _ => {}
    }
}

/// Collects the trait paths from `dyn`/`impl Trait` bounds.
fn collect_bound_paths(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Plus>,
    out: &mut Vec<Vec<String>>,
) {
    for b in bounds {
        if let syn::TypeParamBound::Trait(tb) = b {
            let segs: Vec<String> = tb
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if !segs.is_empty() {
                out.push(segs);
            }
        }
    }
}

/// True for items gated to the test build (`#[cfg(test)]`) or test functions
/// (`#[test]`). Test scaffolding is not architecture, so it is skipped.
fn is_test_gated(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("test") {
            return true;
        }
        if attr.path().is_ident("cfg")
            && let syn::Meta::List(list) = &attr.meta
            && tokens_contain_test(list.tokens.clone())
        {
            return true;
        }
    }
    false
}

/// Recursively checks a token stream for a standalone `test` identifier, so
/// `cfg(test)` and `cfg(all(test, …))` match but `cfg(feature = "test-utils")`
/// does not.
fn tokens_contain_test(tokens: proc_macro2::TokenStream) -> bool {
    for tok in tokens {
        match tok {
            proc_macro2::TokenTree::Ident(id) if id == "test" => return true,
            proc_macro2::TokenTree::Group(g) if tokens_contain_test(g.stream()) => return true,
            _ => {}
        }
    }
    false
}

/// First non-empty line of an item's `///` / `//!` doc-comment, trimmed. This
/// is the seed for the prose layer and keeps `model.json` compact.
fn first_doc(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            let line = s.value().trim().to_string();
            if !line.is_empty() {
                return Some(line);
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn scope(entries: &[(&str, UseTarget)]) -> HashMap<String, UseTarget> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn ws(krate: &str, leaf: &str) -> UseTarget {
        UseTarget::Workspace(Target {
            krate: krate.to_string(),
            leaf: leaf.to_string(),
        })
    }

    fn idents() -> HashMap<String, String> {
        [
            ("regelrecht_law_model", "law-model"),
            ("regelrecht_shared", "shared"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
    }

    fn segs(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bare_name_falls_back_to_current_crate() {
        let t = resolve_path(&segs(&["Foo"]), "engine", &HashMap::new(), &idents()).unwrap();
        assert_eq!(t.krate, "engine");
        assert_eq!(t.leaf, "Foo");
    }

    #[test]
    fn aliased_use_resolves_to_original_leaf_and_crate() {
        // `use regelrecht_law_model::money::Cents as Bedrag;` then a field `Bedrag`.
        let sc = scope(&[("Bedrag", ws("law-model", "Cents"))]);
        let t = resolve_path(&segs(&["Bedrag"]), "engine", &sc, &idents()).unwrap();
        assert_eq!(t.krate, "law-model");
        assert_eq!(t.leaf, "Cents");
    }

    #[test]
    fn externally_imported_name_is_not_leaf_matched_locally() {
        // `use std::io::Error;` must not make a bare `Error` a local edge.
        let sc = scope(&[("Error", UseTarget::External)]);
        assert!(resolve_path(&segs(&["Error"]), "engine", &sc, &idents()).is_none());
    }

    #[test]
    fn crate_qualified_path_is_current_crate() {
        let t = resolve_path(
            &segs(&["crate", "service", "Foo"]),
            "engine",
            &HashMap::new(),
            &idents(),
        )
        .unwrap();
        assert_eq!(t.krate, "engine");
        assert_eq!(t.leaf, "Foo");
    }

    #[test]
    fn cross_crate_path_resolves_via_ident() {
        let t = resolve_path(
            &segs(&["regelrecht_law_model", "money", "Cents"]),
            "engine",
            &HashMap::new(),
            &idents(),
        )
        .unwrap();
        assert_eq!(t.krate, "law-model");
        assert_eq!(t.leaf, "Cents");
    }

    #[test]
    fn unknown_multisegment_path_is_dropped_not_leaf_matched() {
        // `std::fmt::Error` must not resolve to a same-crate `Error`.
        assert!(
            resolve_path(
                &segs(&["std", "fmt", "Error"]),
                "engine",
                &HashMap::new(),
                &idents()
            )
            .is_none()
        );
    }

    #[test]
    fn imported_module_head_keeps_crate_takes_own_leaf() {
        // `use regelrecht_law_model::money;` then `money::Cents`.
        let sc = scope(&[("money", ws("law-model", "money"))]);
        let t = resolve_path(&segs(&["money", "Cents"]), "engine", &sc, &idents()).unwrap();
        assert_eq!(t.krate, "law-model");
        assert_eq!(t.leaf, "Cents");
    }

    #[test]
    fn collect_type_paths_walks_generics_and_dyn() {
        let ty: syn::Type = syn::parse_str("Option<Box<dyn Repo>>").unwrap();
        let mut out = Vec::new();
        collect_type_paths(&ty, &mut out);
        let leaves: Vec<String> = out.iter().map(|p| p.last().unwrap().clone()).collect();
        assert!(leaves.contains(&"Option".to_string()));
        assert!(leaves.contains(&"Box".to_string()));
        assert!(leaves.contains(&"Repo".to_string()));
    }

    #[test]
    fn conflicting_imports_downgrade_to_external() {
        let mut sc = HashMap::new();
        let ci = idents();
        insert_use(
            &mut sc,
            "Foo".into(),
            &segs(&["regelrecht_law_model", "Foo"]),
            "engine",
            &ci,
        );
        insert_use(
            &mut sc,
            "Foo".into(),
            &segs(&["regelrecht_shared", "Foo"]),
            "engine",
            &ci,
        );
        assert_eq!(sc.get("Foo"), Some(&UseTarget::External));
    }
}
