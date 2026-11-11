//! GitHub **domain** layer for the corpus: turns a [`GitHubSource`] into the
//! set of regulation files to load, on top of the shared
//! [`regelrecht_github::GithubClient`] transport.
//!
//! All raw REST talk (Trees/Contents/archive requests, ETag caching, rate
//! limits) lives in `regelrecht-github`. This module keeps only the
//! corpus-specific knowledge: which YAML paths under a source are laws, how to
//! pick the best version per law, and how to extract `implements` lists from a
//! repo archive. Everything here is a free function taking `&GithubClient`, so
//! there is no second GitHub client type and no circular dependency (the crate
//! never learns about corpus types).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use regelrecht_github::GithubClient;

use crate::error::{CorpusError, Result};
use crate::models::GitHubSource;

/// Result of fetching a GitHub source.
#[derive(Debug)]
pub enum FetchResult {
    /// New or updated content was fetched.
    Fetched(Vec<FetchedFile>),
    /// Content has not changed since last fetch (HTTP 304).
    NotModified,
}

/// A fetched file from GitHub.
#[derive(Debug, Clone)]
pub struct FetchedFile {
    pub path: String,
    pub content: String,
}

/// A YAML file discovered via the Trees API: its repo-relative path plus the
/// blob sha the tree listing reported. The sha is the file's content identity
/// — two listings reporting the same sha are byte-identical.
#[derive(Debug, Clone)]
struct TreeFile {
    path: String,
    sha: Option<String>,
}

/// How many repository refs [`TreeListings`] keeps a listing for. One per
/// branch that gets listed: the central corpus, and the branch of each
/// traject in use. Only the central listing is large (tens of thousands of
/// entries); a traject repo holds a handful.
const TREE_LISTINGS_KEPT: usize = 64;

/// The YAML files of a repository ref, as last listed, kept per
/// `(repo, ref)` together with the commit they were listed at.
///
/// A commit's tree never changes, so a listing is reused for as long as the
/// ref still points at that commit: [`list_yaml_files`] asks GitHub for the
/// commit first (one small call) and fetches the tree only when it moved.
/// Listing the central corpus costs seconds, and every traject build and
/// every index refresh lists it again; with this they share one listing.
///
/// Shared by everything built from the same [`crate::GithubSettings`].
/// Reuse needs the commit, which only a caller that can read the ref gets
/// from GitHub, so a listing of a private repository is never handed to
/// someone without access to it.
#[derive(Default)]
pub struct TreeListings {
    /// Least recently used first.
    entries: Mutex<VecDeque<CachedListing>>,
}

struct CachedListing {
    repo: String,
    git_ref: String,
    commit: String,
    files: Arc<[TreeFile]>,
}

impl TreeListings {
    /// The listing of `repo@git_ref` when it was taken at `commit`.
    fn get(&self, repo: &str, git_ref: &str, commit: &str) -> Option<Arc<[TreeFile]>> {
        // A poisoned lock only means another lister panicked; a cache miss
        // is always safe, so recover nothing and list again.
        let mut entries = self.entries.lock().ok()?;
        let at = entries
            .iter()
            .position(|e| e.repo == repo && e.git_ref == git_ref && e.commit == commit)?;
        let entry = entries.remove(at)?;
        let files = Arc::clone(&entry.files);
        entries.push_back(entry);
        drop(entries);
        Some(files)
    }

    /// Keep `files` as the listing of `repo@git_ref` at `commit`, replacing
    /// whatever that ref had before.
    fn insert(&self, repo: &str, git_ref: &str, commit: String, files: Arc<[TreeFile]>) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|e| !(e.repo == repo && e.git_ref == git_ref));
        if entries.len() >= TREE_LISTINGS_KEPT {
            entries.pop_front();
        }
        entries.push_back(CachedListing {
            repo: repo.to_string(),
            git_ref: git_ref.to_string(),
            commit,
            files,
        });
    }
}

/// Fetch the laws matching the given `$id` set from a GitHub source, or every
/// law when `law_ids` is `None`.
///
/// Uses the Trees API (1 call) to discover file paths, matches them against
/// `law_ids` by extracting the law directory name from the path
/// (`{base}/{layer}/{law_id}/{date}.yaml`), picks the best version per law
/// (latest `valid_from` ≤ today), and fetches only those files.
///
/// # Errors
///
/// When listing the tree fails. A file that fails to fetch is skipped.
pub async fn fetch_source_filtered(
    client: &GithubClient,
    listings: &TreeListings,
    source: &GitHubSource,
    token: Option<&str>,
    law_ids: Option<&HashSet<String>>,
    today: &str,
) -> Result<FetchResult> {
    if law_ids.is_some_and(HashSet::is_empty) {
        return Ok(FetchResult::Fetched(Vec::new()));
    }

    let base_path = source.path.as_deref().unwrap_or("");

    let Some(all_paths) = list_yaml_files(
        client,
        listings,
        &source.full_repo(),
        source.effective_ref(),
        token,
    )
    .await?
    else {
        return Ok(FetchResult::NotModified);
    };

    let best_per_law = group_best_versions(&all_paths, base_path, law_ids, today);

    tracing::info!(
        matched = best_per_law.len(),
        requested = law_ids.map(HashSet::len),
        "fetching laws from GitHub"
    );

    let mut files = Vec::new();
    for file in best_per_law.values() {
        match client
            .fetch_file_raw(
                &source.full_repo(),
                source.effective_ref(),
                &file.path,
                token,
            )
            .await
        {
            Ok(content) => files.push(FetchedFile {
                path: file.path.clone(),
                content,
            }),
            Err(e) => {
                tracing::warn!(path = %file.path, error = %e, "Failed to fetch file, skipping");
            }
        }
    }

    Ok(FetchResult::Fetched(files))
}

/// What one Trees call says about a GitHub source: its laws, and which of
/// them have a notes sidecar.
#[derive(Debug)]
pub struct SourceListing {
    /// `(law_id, repo_path, blob_sha)` for the best version of each law; the
    /// sha is the file's content identity from the tree listing, so callers
    /// can detect content change across enumerations without fetching bodies.
    pub laws: Vec<(String, String, Option<String>)>,
    /// The law ids with a notes sidecar, at the repository root or at the
    /// legacy location under the source's subpath (see
    /// [`crate::annotation_sidecar`]). The listing is complete (a truncated
    /// tree is an error), so a law missing here has no sidecar in this tree.
    pub sidecars: HashSet<String>,
}

/// Enumerate every law in a source via the Trees API (1 call), selecting the
/// best version per law — WITHOUT fetching any file content. This is the
/// cheap enumeration the lightweight corpus index is built from. `None` when
/// the tree is unchanged since this client's previous listing (HTTP 304).
///
/// # Errors
///
/// When the Trees call fails: a transport error, a non-success status, or a
/// truncated listing.
pub async fn list_source_law_paths(
    client: &GithubClient,
    listings: &TreeListings,
    source: &GitHubSource,
    token: Option<&str>,
    today: &str,
) -> Result<Option<SourceListing>> {
    let base_path = source.path.as_deref().unwrap_or("");
    let Some(all_paths) = list_yaml_files(
        client,
        listings,
        &source.full_repo(),
        source.effective_ref(),
        token,
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(SourceListing {
        laws: group_best_versions(&all_paths, base_path, None, today)
            .into_iter()
            .map(|(law_id, file)| (law_id, file.path, file.sha))
            .collect(),
        sidecars: sidecar_law_ids(&all_paths, base_path),
    }))
}

/// The law ids whose notes sidecar appears in `all_paths`: either
/// `annotations/{law_id}/annotations.yaml` at the repository root, or the
/// same path under `base_path` where earlier editor versions wrote it.
fn sidecar_law_ids(all_paths: &[TreeFile], base_path: &str) -> HashSet<String> {
    let legacy_prefix = format!("{base_path}/");
    all_paths
        .iter()
        .filter_map(|file| {
            let path = file.path.as_str();
            let rel = match path.strip_prefix(&legacy_prefix) {
                Some(rel) if !base_path.is_empty() => rel,
                _ => path,
            };
            match rel.split('/').collect::<Vec<_>>().as_slice() {
                ["annotations", law_id, "annotations.yaml"] => Some((*law_id).to_string()),
                _ => None,
            }
        })
        .collect()
}

/// Bulk `implements` scan via the repo archive: download the tarball in one
/// request (through the shared client) and return each YAML law's `implements`
/// list — `(repo-relative path, implements)` pairs for `.yaml`/`.yml` files,
/// the archive's top-level `{owner}-{repo}-{sha}/` component stripped.
///
/// Bodies are parsed and DISCARDED one at a time during extraction (see
/// [`extract_implements_from_tar_gz`]) so a large corpus archive never
/// materialises in memory at once. gunzip + untar + parse are synchronous and
/// CPU-bound, so they run on a blocking thread off the async runtime.
pub async fn fetch_archive_implements(
    client: &GithubClient,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
) -> Result<Vec<(String, Vec<String>)>> {
    let bytes = client.fetch_tarball(repo, git_ref, token).await?;
    let files = tokio::task::spawn_blocking(move || extract_implements_from_tar_gz(bytes.as_ref()))
        .await
        .map_err(|e| CorpusError::Config(format!("archive extract task panicked: {e}")))??;
    Ok(files)
}

/// List the YAML files of `repo@git_ref`, keeping the blob sha each entry
/// reported. Reuses the listing in `listings` while the ref still points at
/// the commit it was taken at (see [`TreeListings`]); otherwise lists the
/// tree at that commit with one Trees call and keeps it. Returns `None` only
/// when the ref does not resolve to a commit and the fallback listing by ref
/// gets a 304 (unchanged since this client's previous listing). Callers narrow
/// to their source's subpath themselves: [`group_best_versions`] skips paths
/// outside it, and [`sidecar_law_ids`] needs the repository root as well.
async fn list_yaml_files(
    client: &GithubClient,
    listings: &TreeListings,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
) -> Result<Option<Arc<[TreeFile]>>> {
    let Some(commit) =
        crate::timing::measure("gh_commit", client.commit_sha(repo, git_ref, token)).await?
    else {
        // No such ref: the Trees call reports it the way it always has.
        return Ok(list_tree_yaml(client, repo, git_ref, token)
            .await?
            .map(Arc::from));
    };
    if let Some(files) = listings.get(repo, git_ref, &commit) {
        tracing::debug!(repo = %repo, git_ref = %git_ref, commit = %commit, "tree listing reused");
        return Ok(Some(files));
    }
    let entries = crate::timing::measure(
        "gh_tree",
        client.list_commit_tree_files(repo, &commit, token),
    )
    .await?;
    let files: Arc<[TreeFile]> = Arc::from(yaml_files(entries));
    listings.insert(repo, git_ref, commit, Arc::clone(&files));
    Ok(Some(files))
}

/// One conditional Trees call: the YAML files of `repo` at `git_ref`, or
/// `None` on a 304.
async fn list_tree_yaml(
    client: &GithubClient,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
) -> Result<Option<Vec<TreeFile>>> {
    let Some(entries) =
        crate::timing::measure("gh_tree", client.list_tree_files(repo, git_ref, token)).await?
    else {
        return Ok(None);
    };
    Ok(Some(yaml_files(entries)))
}

/// The YAML files among a tree listing's entries.
fn yaml_files(entries: Vec<regelrecht_github::TreeEntryFile>) -> Vec<TreeFile> {
    let yaml_files: Vec<TreeFile> = entries
        .into_iter()
        .filter(|e| e.path.ends_with(".yaml"))
        .map(|e| TreeFile {
            path: e.path,
            sha: e.sha,
        })
        .collect();
    tracing::debug!(count = yaml_files.len(), "Found YAML files in tree");
    yaml_files
}

/// Group repo-relative YAML files by `law_id` (the directory name), keeping the
/// best version per law (closest valid date ≤ today, else latest). `filter`,
/// when set, restricts to those law_ids. Path format:
/// `{base_path}/{layer}/{law_id}/{date}.yaml`.
fn group_best_versions(
    all_paths: &[TreeFile],
    base_path: &str,
    filter: Option<&HashSet<String>>,
    today: &str,
) -> HashMap<String, TreeFile> {
    let prefix = if base_path.is_empty() {
        String::new()
    } else {
        format!("{}/", base_path)
    };
    let mut best_per_law: HashMap<String, TreeFile> = HashMap::new();

    for file in all_paths {
        let path = &file.path;
        let rel = if prefix.is_empty() {
            path.as_str()
        } else {
            match path.strip_prefix(&prefix) {
                Some(r) => r,
                None => continue,
            }
        };

        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() < 3 {
            continue;
        }

        // Annotations are persisted at the reserved
        // `annotations/{law_id}/annotations.yaml` path at the repository root
        // (RFC-018), which falls inside this source only when it has no
        // subpath; a source with a subpath may still hold the legacy copy
        // under it. That shape collides with the law-file convention
        // `{layer}/{law_id}/{date}.yaml`, so without this guard the annotation
        // file is indexed as a phantom law whose body is the annotation YAML —
        // the law then opens to an empty editor ("Geen items"). Skip the
        // annotations subtree entirely.
        if parts[0] == "annotations" {
            continue;
        }

        let law_id = parts[parts.len() - 2];
        if let Some(f) = filter
            && !f.contains(law_id)
        {
            continue;
        }

        let new_date = crate::source_map::extract_date_from_path(rel);

        if let Some(existing) = best_per_law.get(law_id) {
            let existing_date = crate::source_map::extract_date_from_path(&existing.path);

            let new_wins = crate::source_map::pick_best_version(
                existing_date.as_deref(),
                new_date.as_deref(),
                today,
            );

            if new_wins {
                best_per_law.insert(law_id.to_string(), file.clone());
            }
        } else {
            best_per_law.insert(law_id.to_string(), file.clone());
        }
    }

    best_per_law
}

/// Stream a gzipped tar produced by GitHub's tarball endpoint and return
/// `(repo-relative path, implements list)` for every `.yaml`/`.yml` file
/// ([`each_yaml_in_tar_gz`]).
///
/// Each body is parsed for `implements` and dropped before the next entry —
/// so peak memory is one law body plus the (tiny) implements result, never
/// the whole decompressed corpus.
fn extract_implements_from_tar_gz(bytes: &[u8]) -> Result<Vec<(String, Vec<String>)>> {
    let mut out = Vec::new();
    each_yaml_in_tar_gz(bytes, |rel, content| {
        let implements = crate::source_map::collect_law_implements(&content);
        out.push((rel.to_string(), implements));
        // `content` dropped here — bodies never accumulate.
    })?;
    Ok(out)
}

/// Walk a gzipped tar produced by GitHub's tarball endpoint and hand every
/// `.yaml`/`.yml` file to `each`, as `(repo-relative path, text)`, one at a
/// time. The archive nests everything under a single top-level
/// `{owner}-{repo}-{sha}/` directory; that first component is stripped.
/// Directories, non-YAML files, and non-UTF-8 bodies are skipped rather
/// than failing the whole walk. Returns that top-level directory's name,
/// which ends in the short sha of the commit, if the archive had an entry.
pub fn each_yaml_in_tar_gz(
    bytes: &[u8],
    mut each: impl FnMut(&str, String),
) -> Result<Option<String>> {
    use std::io::Read;
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| CorpusError::Git(format!("failed to read archive entries: {e}")))?;
    let mut top = None;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| CorpusError::Git(format!("failed to read archive entry: {e}")))?;
        let kind = entry.header().entry_type();
        // GitHub's archive opens with a `pax_global_header` entry (the commit
        // sha as a pax comment), which is not the top-level directory.
        if !(kind.is_dir() || kind == tar::EntryType::Regular) {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| CorpusError::Git(format!("archive entry has no path: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");
        if top.is_none() {
            top = path.split('/').next().map(str::to_string);
        }
        if kind.is_dir() {
            continue;
        }
        // Strip the archive's single top-level directory component.
        let Some((_, rel)) = path.split_once('/') else {
            continue;
        };
        if !(rel.ends_with(".yaml") || rel.ends_with(".yml")) {
            continue;
        }
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_err() {
            tracing::debug!(path = %rel, "archive entry is not valid UTF-8; skipping");
            continue;
        }
        each(rel, content);
    }
    Ok(top)
}

#[cfg(test)]
mod tests {
    use super::{TreeFile, group_best_versions};
    use std::collections::HashMap;

    fn sorted_ids(map: &HashMap<String, TreeFile>) -> Vec<String> {
        let mut ids: Vec<String> = map.keys().cloned().collect();
        ids.sort();
        ids
    }

    // A saved annotation lives at `annotations/{law_id}/annotations.yaml` in
    // the traject's own repo. That path shape collides with the law-file
    // convention `{layer}/{law_id}/{date}.yaml`, so without an explicit guard
    // the indexer registers the annotation file as a phantom law whose
    // "content" is the annotation YAML — the law then opens to an empty editor
    // ("Geen items"). Annotations must never be indexed as laws.
    #[test]
    fn annotation_files_are_not_indexed_as_laws() {
        let paths = vec![
            TreeFile {
                path: "regulation/nl/wet/wet_op_de_zorgtoeslag/2026-01-01.yaml".to_string(),
                sha: Some("abc123".to_string()),
            },
            TreeFile {
                path: "annotations/zorgtoeslagwet/annotations.yaml".to_string(),
                sha: Some("def456".to_string()),
            },
        ];
        let best = group_best_versions(&paths, "", None, "2026-06-01");
        assert_eq!(sorted_ids(&best), vec!["wet_op_de_zorgtoeslag".to_string()]);
        assert!(
            !best.contains_key("zorgtoeslagwet"),
            "annotation file was mis-indexed as law 'zorgtoeslagwet'"
        );
    }

    #[test]
    fn sidecars_are_found_at_the_root_and_at_the_legacy_location() {
        let file = |path: &str| TreeFile {
            path: path.to_string(),
            sha: None,
        };
        let paths = vec![
            file("annotations/wet_root/annotations.yaml"),
            file("regulation/nl/annotations/wet_legacy/annotations.yaml"),
            // Not a sidecar: a law file, a sidecar outside the source's
            // subpath, and a file next to a sidecar.
            file("regulation/nl/wet/wet_root/2025-01-01.yaml"),
            file("elders/annotations/wet_elders/annotations.yaml"),
            file("annotations/wet_root/extra.yaml"),
        ];
        let mut found: Vec<String> = super::sidecar_law_ids(&paths, "regulation/nl")
            .into_iter()
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec!["wet_legacy".to_string(), "wet_root".to_string()]
        );

        // Without a subpath only the root location exists.
        let found = super::sidecar_law_ids(&paths, "");
        assert_eq!(
            found,
            std::collections::HashSet::from(["wet_root".to_string()])
        );
    }

    // Both routes into `pick_best_version` must read a filename the same way.
    // The tree route used to hand it the bare stem, so `2025-01-01-concept`
    // sorted above `2025-01-01` and the concept won — while the local scan,
    // which validates the stem as a date, dropped it. Same repo, two answers.
    #[test]
    fn undated_stem_loses_to_a_dated_file_on_both_routes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let law_dir = dir.path().join("wet").join("test_wet");
        std::fs::create_dir_all(&law_dir).expect("create law dir");
        let body = "$id: test_wet\nname: Test\nregulatory_layer: WET\narticles: []\n";
        for stem in ["2025-01-01", "2025-01-01-concept"] {
            std::fs::write(law_dir.join(format!("{stem}.yaml")), body).expect("write law");
        }

        let source = crate::models::Source {
            id: "local".to_string(),
            name: "Local".to_string(),
            source_type: crate::models::SourceType::Local {
                local: crate::models::LocalSource {
                    path: dir.path().to_path_buf(),
                },
            },
            scopes: vec![],
            priority: 1,
            auth_ref: None,
            strict_auth: false,
            enriched: None,
            load_all: false,
            library_only: false,
        };
        let mut map = crate::source_map::SourceMap::new("2026-06-01");
        map.load_source(&source).expect("load local source");
        let local_pick = map
            .get_law("test_wet")
            .expect("law loaded")
            .file_path
            .clone();

        let paths: Vec<TreeFile> = ["2025-01-01", "2025-01-01-concept"]
            .iter()
            .map(|stem| TreeFile {
                path: format!("wet/test_wet/{stem}.yaml"),
                sha: None,
            })
            .collect();
        let tree_pick = group_best_versions(&paths, "", None, "2026-06-01")
            .get("test_wet")
            .expect("law indexed")
            .path
            .clone();

        assert!(
            local_pick.ends_with("2025-01-01.yaml"),
            "local scan picked {local_pick}"
        );
        assert!(
            tree_pick.ends_with("2025-01-01.yaml"),
            "tree scan picked {tree_pick}"
        );
    }
}
