use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use walkdir::WalkDir;

use crate::error::{CorpusError, Result};
use crate::models::{Source, SourceType};

/// A loaded law with its source provenance.
#[derive(Debug, Clone)]
pub struct LoadedLaw {
    /// The law's `$id` field.
    pub law_id: String,
    /// The law's `name` field (human-readable title), if present.
    pub name: Option<String>,
    /// Resolved human-readable display name, computed once at load time.
    /// Equals [`name`](Self::name) for laws with a literal `name:` field;
    /// for `name: '#output_ref'` laws this is the referenced action's
    /// scalar value (see [`resolve_display_name`]). `None` when nothing
    /// resolves, or for metadata-only entries whose body hasn't been
    /// fetched. Precomputed so list endpoints don't re-scan (or fully
    /// re-parse) every law body on every request.
    pub display_name: Option<String>,
    /// `$id`s of the higher laws this law's articles declare they
    /// `implements` (the IoC link), computed once at load time (see
    /// [`collect_law_implements`]). Empty for metadata-only entries.
    /// Precomputed so "who implements law X" reverse lookups don't
    /// re-parse every law body per request.
    pub implements: Vec<String>,
    /// Whether any article carries a `machine_readable` section, computed once
    /// at load time. `false` for metadata-only entries, whose body is unknown.
    pub has_machine_readable: bool,
    /// The raw YAML content.
    pub yaml_content: String,
    /// Path to the source file. For local sources this is the absolute path
    /// on disk; for fetched (e.g. GitHub) sources this is the path inside
    /// the upstream repository.
    pub file_path: String,
    /// Path to the source file *relative to the source root* — i.e. the
    /// portion of [`file_path`](Self::file_path) below the source's own
    /// root directory. This is what backends use to address the file via
    /// [`RepoBackend::write_file`](crate::backend::RepoBackend::write_file)
    /// and friends, and avoids the structural-depth heuristic that breaks
    /// when a source root is configured at an unusual location.
    pub relative_path: String,
    /// Blob sha of the law file as reported by the source's enumeration
    /// (the GitHub Trees API listing), for metadata-only entries. This is
    /// the file's content identity: two enumerations reporting the same
    /// sha are guaranteed to have byte-identical content, so callers can
    /// reuse content-derived data across index snapshots without
    /// re-fetching the body. `None` for eagerly-loaded entries (local
    /// sources — the content is already present and parsed) and after
    /// [`SourceMap::update_yaml_content`] (the in-memory content no
    /// longer matches the enumerated blob).
    pub content_sha: Option<String>,
    /// ID of the source that provided this law.
    pub source_id: String,
    /// Name of the source that provided this law.
    pub source_name: String,
    /// Priority of the source (lower = higher priority).
    pub source_priority: u32,
}

impl LoadedLaw {
    /// Whether this law's body has been fetched. Metadata-only index entries
    /// (from path enumeration) carry an empty `yaml_content` until the law is
    /// opened and its content lazily fetched.
    ///
    /// Empty `yaml_content` is a safe "not yet fetched" sentinel only because a
    /// corpus YAML file is never legitimately empty — it always carries at
    /// least a `$id`. A genuinely-empty file would report `false` here and be
    /// re-fetched on each open until the read-your-writes overlay caches it
    /// (one extra read per pod, then the overlay short-circuits — see
    /// `TrajectCorpus::law_yaml`), so the contract is "non-empty == loaded".
    pub fn is_loaded(&self) -> bool {
        !self.yaml_content.is_empty()
    }
}

/// Aggregates laws from multiple sources with priority-based conflict resolution.
///
/// When multiple sources provide a law with the same `$id`, the source with the
/// lowest priority value wins. Equal priority with the same `$id` is an error.
#[derive(Debug)]
pub struct SourceMap {
    /// Laws indexed by `$id`, with provenance metadata. Holds the single
    /// "best for today" version per id (see [`SourceMap::insert`]) — the
    /// shape every existing single-version caller expects.
    laws: HashMap<String, LoadedLaw>,
    /// *All* versions of each law, indexed by `$id`. Where [`laws`](Self::laws)
    /// collapses an id to one entry, this keeps every version so the scenario
    /// dependency loader can hand the engine the full set and let it pick the
    /// version in force on the scenario's calculation date — not just today's.
    /// See [`SourceMap::get_law_versions`].
    versions: HashMap<String, Vec<LoadedLaw>>,
    /// Conflicts that were resolved (for reporting).
    resolved_conflicts: Vec<ConflictResolution>,
    /// The date "currently in force" is measured against when collapsing a
    /// law's versions to one. Supplied by the caller rather than read off a
    /// clock here: this crate deliberately carries no time dependency, and the
    /// answer is a Dutch calendar date (`regelrecht_shared::dates`), not the
    /// timezone of whichever machine happens to run the scan.
    reference_date: String,
    /// How [`laws`](Self::laws) picks a law's current version when sources
    /// overlap. `false` (the default): the highest-priority source supplies it,
    /// whatever the dates. `true`: the version in force on `reference_date`
    /// wins across sources, and priority only decides between two sources that
    /// supply the same date. See [`Self::prefer_version_in_force`].
    version_in_force_first: bool,
}

/// Record of a conflict that was resolved by priority.
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub law_id: String,
    pub winner_source_id: String,
    pub winner_priority: u32,
    pub loser_source_id: String,
    pub loser_priority: u32,
}

impl SourceMap {
    /// Create an empty source map.
    pub fn new(reference_date: impl Into<String>) -> Self {
        Self {
            laws: HashMap::new(),
            versions: HashMap::new(),
            resolved_conflicts: Vec::new(),
            reference_date: reference_date.into(),
            version_in_force_first: false,
        }
    }

    /// Let the date decide a law's current version across sources, instead of
    /// source priority.
    ///
    /// RFC-010's priority settles which source supplies a *version*; a version
    /// for another date is a different version, not the same law twice. So a
    /// source with an older version no longer hides a newer one from a
    /// lower-priority source: the version in force on the reference date is
    /// current, and priority still picks between two sources for one date (a
    /// local override of the same version keeps working). The version set the
    /// engine loads ([`Self::get_law_versions`]) already worked this way; this
    /// makes the single current version agree with it.
    ///
    /// Re-derives every current version from the version set, so it can be
    /// called after loading. Laws inserted later follow the same rule.
    pub fn prefer_version_in_force(&mut self) {
        self.version_in_force_first = true;
        let ids: Vec<String> = self.versions.keys().cloned().collect();
        for id in ids {
            self.refresh_current_version(&id);
        }
    }

    /// Set `laws[law_id]` to the version in force on the reference date, taken
    /// from the merged version set (where priority already settled each date).
    fn refresh_current_version(&mut self, law_id: &str) {
        let Some(entries) = self.versions.get(law_id) else {
            return;
        };
        let mut best: Option<&LoadedLaw> = None;
        for candidate in entries {
            let wins = match best {
                None => true,
                Some(current) => pick_best_version(
                    extract_date_from_path(&current.file_path).as_deref(),
                    extract_date_from_path(&candidate.file_path).as_deref(),
                    &self.reference_date,
                ),
            };
            if wins {
                best = Some(candidate);
            }
        }
        if let Some(best) = best.cloned() {
            self.laws.insert(law_id.to_string(), best);
        }
    }

    /// The date this map resolves "currently in force" against.
    pub fn reference_date(&self) -> &str {
        &self.reference_date
    }

    /// Load laws from a single source directory.
    ///
    /// Scans the directory for `.yaml` files, extracts the `$id` field,
    /// and adds them to the map with conflict resolution.
    pub fn load_source(&mut self, source: &Source) -> Result<usize> {
        let path = match &source.source_type {
            SourceType::Local { local } => &local.path,
            SourceType::GitHub { .. } => {
                return Err(CorpusError::Config(
                    "GitHub sources must be fetched before loading into SourceMap".to_string(),
                ));
            }
        };

        self.load_from_directory(path, source)
    }

    /// Load all YAML files from a directory into the source map.
    pub fn load_from_directory(&mut self, dir: &Path, source: &Source) -> Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!(
                        path = ?err.path(),
                        error = %err,
                        "Failed to read directory entry, skipping"
                    );
                    None
                }
            })
        {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "yaml") {
                continue;
            }

            let content = std::fs::read_to_string(path).map_err(|e| {
                CorpusError::Config(format!(
                    "Failed to read {} from source '{}': {}",
                    path.display(),
                    source.id,
                    e
                ))
            })?;

            let law_id = match extract_law_id(&content) {
                Some(id) => id,
                None => continue, // Skip files without $id
            };

            let name = extract_law_name(&content);
            let (display_name, implements, has_machine_readable) = derive_loaded_fields(&content);

            // Compute the path relative to the source root so that writes
            // can address the file via the backend without re-deriving the
            // structural location from a depth heuristic.
            let relative_path = path
                .strip_prefix(dir)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());

            let loaded = LoadedLaw {
                law_id: law_id.clone(),
                name,
                display_name,
                implements,
                has_machine_readable,
                yaml_content: content,
                file_path: path.display().to_string(),
                relative_path,
                content_sha: None,
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                source_priority: source.priority,
            };

            self.insert(loaded)?;
            count += 1;
        }

        Ok(count)
    }

    /// Insert a law into the map, resolving conflicts by priority.
    ///
    /// When two entries share the same `$id` and priority:
    /// - **Same source**: multiple versions of one law. Keep the version whose
    ///   `valid_from` date (from the filename) is closest to today without
    ///   exceeding it. If no version is currently valid, keep the latest.
    /// - **Different sources**: this is still a hard error (ambiguous ownership).
    ///
    /// After [`Self::prefer_version_in_force`], the current version comes from
    /// the merged version set instead; see there.
    fn insert(&mut self, law: LoadedLaw) -> Result<()> {
        let law_id = law.law_id.clone();

        if self.version_in_force_first {
            // Two sources at equal priority stay an error, as RFC-010 asks,
            // whichever of their versions happens to be current.
            if let Some(other) = self.versions.get(&law_id).and_then(|vs| {
                vs.iter().find(|v| {
                    v.source_priority == law.source_priority && v.source_id != law.source_id
                })
            }) {
                return Err(CorpusError::Config(format!(
                    "Conflict: law '{}' provided by both '{}' and '{}' with equal priority {}",
                    law_id, other.source_id, law.source_id, law.source_priority
                )));
            }
            if let Some(existing) = self.laws.get(&law_id) {
                if existing.source_id != law.source_id {
                    let (winner, loser) = if law.source_priority < existing.source_priority {
                        (&law, existing)
                    } else {
                        (existing, &law)
                    };
                    self.resolved_conflicts.push(ConflictResolution {
                        law_id: law_id.clone(),
                        winner_source_id: winner.source_id.clone(),
                        winner_priority: winner.source_priority,
                        loser_source_id: loser.source_id.clone(),
                        loser_priority: loser.source_priority,
                    });
                }
            }
            self.insert_version(law);
            self.refresh_current_version(&law_id);
            return Ok(());
        }

        // Record every version (independent of the single-best collapse below)
        // so the scenario loader can feed the engine the full set.
        self.insert_version(law.clone());

        if let Some(existing) = self.laws.get(&law_id) {
            if existing.source_priority == law.source_priority {
                // Same source with multiple versions → pick best version
                if existing.source_id == law.source_id {
                    let existing_date = extract_date_from_path(&existing.file_path);
                    let new_date = extract_date_from_path(&law.file_path);

                    let new_wins = pick_best_version(
                        existing_date.as_deref(),
                        new_date.as_deref(),
                        &self.reference_date,
                    );

                    if new_wins {
                        tracing::debug!(
                            law_id = %law_id,
                            kept = %law.file_path,
                            dropped = %existing.file_path,
                            "same-source version conflict resolved"
                        );
                        self.laws.insert(law_id, law);
                    } else {
                        tracing::debug!(
                            law_id = %law_id,
                            kept = %existing.file_path,
                            dropped = %law.file_path,
                            "same-source version conflict resolved"
                        );
                    }
                    return Ok(());
                }

                return Err(CorpusError::Config(format!(
                    "Conflict: law '{}' provided by both '{}' and '{}' with equal priority {}",
                    law_id, existing.source_id, law.source_id, law.source_priority
                )));
            }

            if law.source_priority < existing.source_priority {
                // New law wins (lower priority value = higher priority)
                self.resolved_conflicts.push(ConflictResolution {
                    law_id: law_id.clone(),
                    winner_source_id: law.source_id.clone(),
                    winner_priority: law.source_priority,
                    loser_source_id: existing.source_id.clone(),
                    loser_priority: existing.source_priority,
                });
                self.laws.insert(law_id, law);
            } else {
                // Existing law wins
                self.resolved_conflicts.push(ConflictResolution {
                    law_id: law_id.clone(),
                    winner_source_id: existing.source_id.clone(),
                    winner_priority: existing.source_priority,
                    loser_source_id: law.source_id.clone(),
                    loser_priority: law.source_priority,
                });
            }
        } else {
            self.laws.insert(law_id, law);
        }

        Ok(())
    }

    /// Record one version of a law in the per-id version set ([`Self::versions`]).
    ///
    /// Versions are keyed by their filename date (`…/{id}/{date}.yaml`). Two
    /// sources providing the same `$id` for the *same* date collapse to the
    /// higher-priority source's body (lower `source_priority` wins; ties keep
    /// the incumbent, mirroring the single-best map's "existing wins on equal
    /// priority"). A date present in only one source is always kept — so the
    /// engine sees, e.g., a local always-in-force version *and* central's
    /// future-dated one, and `select_in` can pick the one in force on the
    /// scenario date. Undated files share a single slot, resolved the same way.
    /// Order-independent: the outcome is the same whichever source is indexed
    /// first. Entries are kept sorted newest-first (undated last) for
    /// deterministic output; the engine re-sorts on load regardless.
    ///
    /// The filename date is a proxy for the body's `valid_from` (the corpus
    /// convention is `{id}/{valid_from}.yaml`). The engine ultimately re-dedups
    /// the loaded set by the body `valid_from` field, so a filename/body
    /// mismatch can at worst pass a redundant body the engine collapses — it
    /// never drops a genuinely distinct `valid_from`.
    fn insert_version(&mut self, law: LoadedLaw) {
        let new_date = extract_date_from_path(&law.file_path);
        let entries = self.versions.entry(law.law_id.clone()).or_default();

        if let Some(slot) = entries
            .iter_mut()
            .find(|e| extract_date_from_path(&e.file_path) == new_date)
        {
            if law.source_priority < slot.source_priority {
                *slot = law;
            }
        } else {
            entries.push(law);
        }

        // Newest date first; undated entries (None) sort last. This re-sorts on
        // every insertion — O(N) key allocations per insert, O(N²) to build a
        // law's set one version at a time — negligible at the corpus's 1–5
        // versions per law. If version counts ever grow large, prefer a single
        // deferred sort in `get_law_versions` instead.
        entries.sort_by_cached_key(|e| std::cmp::Reverse(extract_date_from_path(&e.file_path)));
    }

    /// Load a single fetched file (from GitHub or other remote) into the map.
    ///
    /// `file_path` is the path inside the upstream repository (e.g.
    /// `regulation/nl/wet/my_law/2025.yaml`). `source_subpath`, when set,
    /// is the in-repo prefix that the source is rooted at — it is stripped
    /// to compute the source-root-relative path stored on `LoadedLaw`.
    pub fn load_fetched_file(
        &mut self,
        content: &str,
        file_path: &str,
        source_subpath: Option<&str>,
        source_id: &str,
        source_name: &str,
        source_priority: u32,
    ) -> Result<bool> {
        let law_id = match extract_law_id(content) {
            Some(id) => id,
            None => return Ok(false),
        };

        let name = extract_law_name(content);
        let (display_name, implements, has_machine_readable) = derive_loaded_fields(content);

        // Strip the source's in-repo subpath so the stored relative path is
        // relative to the source root, matching the on-disk layout the
        // backend writes to.
        let relative_path = match source_subpath {
            Some(sub) if !sub.is_empty() => {
                let trimmed = sub.trim_end_matches('/');
                file_path
                    .strip_prefix(&format!("{trimmed}/"))
                    .unwrap_or(file_path)
                    .to_string()
            }
            _ => file_path.to_string(),
        };

        let loaded = LoadedLaw {
            law_id: law_id.clone(),
            name,
            display_name,
            implements,
            has_machine_readable,
            yaml_content: content.to_string(),
            file_path: file_path.to_string(),
            relative_path,
            content_sha: None,
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            source_priority,
        };

        self.insert(loaded)?;
        Ok(true)
    }

    /// Insert a metadata-only entry for a law discovered by path enumeration,
    /// without fetching its content. `yaml_content` is left empty as the
    /// "not yet loaded" sentinel — callers lazily fetch the body on first read
    /// via the source's backend, using the `relative_path` stored here.
    ///
    /// `law_id` comes from the directory name (the corpus convention is
    /// `{base}/{layer}/{law_id}/{date}.yaml`, and the directory name equals
    /// the law's `$id`). `file_path` is the in-repo path; `source_subpath` is
    /// stripped to produce the source-root-relative path the backend reads.
    /// `content_sha` is the blob sha the enumeration reported for the file
    /// (see [`LoadedLaw::content_sha`]), when available.
    #[allow(clippy::too_many_arguments)]
    pub fn load_metadata_entry(
        &mut self,
        law_id: &str,
        file_path: &str,
        source_subpath: Option<&str>,
        source_id: &str,
        source_name: &str,
        source_priority: u32,
        content_sha: Option<&str>,
    ) -> Result<()> {
        let relative_path = match source_subpath {
            Some(sub) if !sub.is_empty() => {
                let trimmed = sub.trim_end_matches('/');
                file_path
                    .strip_prefix(&format!("{trimmed}/"))
                    .unwrap_or(file_path)
                    .to_string()
            }
            _ => file_path.to_string(),
        };

        self.insert(LoadedLaw {
            law_id: law_id.to_string(),
            name: None,
            display_name: None,
            implements: Vec::new(),
            has_machine_readable: false,
            yaml_content: String::new(),
            file_path: file_path.to_string(),
            relative_path,
            content_sha: content_sha.map(|s| s.to_string()),
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            source_priority,
        })
    }

    /// Get all loaded laws.
    pub fn laws(&self) -> impl Iterator<Item = &LoadedLaw> {
        self.laws.values()
    }

    /// Every version of every law, from every source that supplies one.
    ///
    /// Unlike [`laws`](Self::laws), which holds one winner per `$id`, this
    /// includes a version from a lower-priority source when it supplies a date
    /// the winning source does not have; the engine loads those too (see
    /// [`get_law_versions`](Self::get_law_versions)).
    pub fn all_versions(&self) -> impl Iterator<Item = &LoadedLaw> {
        self.versions.values().flatten()
    }

    /// Get a specific law by ID.
    pub fn get_law(&self, law_id: &str) -> Option<&LoadedLaw> {
        self.laws.get(law_id)
    }

    /// Get *all* versions of a law by ID, newest-first (undated last).
    ///
    /// Unlike [`Self::get_law`] (one "best for today" entry), this returns the
    /// full version set across sources so the scenario dependency loader can
    /// hand the engine every version and let `select_in` pick the one in force
    /// on the scenario's calculation date. Returns `None` only when no version
    /// of the id is indexed. Bodies of metadata-only (GitHub) entries are empty
    /// until lazily fetched via each entry's `relative_path`.
    pub fn get_law_versions(&self, law_id: &str) -> Option<&[LoadedLaw]> {
        self.versions.get(law_id).map(Vec::as_slice)
    }

    /// Update the cached YAML content for an existing law. Used after the
    /// editor persists an edit through [`crate::backend::RepoBackend`], so
    /// subsequent GETs (and dependency walks) see the new text without
    /// waiting for a full corpus reload. Returns `true` if the law was
    /// present and updated, `false` otherwise.
    ///
    /// Only `yaml_content` (and the content-derived fields `name`,
    /// `display_name` and `implements`) is updated — `$id`, `file_path`,
    /// source provenance, and priority are stable across an edit and are
    /// left untouched. If the caller writes a new file under a different
    /// `$id` that's a different operation (unsupported via this hook).
    pub fn update_yaml_content(&mut self, law_id: &str, new_content: String) -> bool {
        // Parse only once the law is known to be present — an unknown id is a
        // no-op, so don't pay for header parsing on that path. The borrow on
        // `laws` is released at the end of the block so the `versions` entry
        // (which reuses the derived fields) can be updated next.
        let (file_path, name, display_name, implements, has_machine_readable) = {
            let Some(law) = self.laws.get_mut(law_id) else {
                return false;
            };
            let name = extract_law_name(&new_content);
            let (display_name, implements, has_machine_readable) =
                derive_loaded_fields(&new_content);
            law.name = name.clone();
            law.display_name = display_name.clone();
            law.implements = implements.clone();
            law.has_machine_readable = has_machine_readable;
            law.yaml_content = new_content.clone();
            // The in-memory content no longer matches the blob the source
            // enumeration reported — drop the stale content identity.
            law.content_sha = None;
            (
                law.file_path.clone(),
                name,
                display_name,
                implements,
                has_machine_readable,
            )
        };

        // Keep the matching version entry coherent with the edited body so the
        // version-set loader (`get_law_versions`) reflects read-your-writes too.
        if let Some(entries) = self.versions.get_mut(law_id) {
            if let Some(v) = entries.iter_mut().find(|v| v.file_path == file_path) {
                v.name = name;
                v.display_name = display_name;
                v.implements = implements;
                v.has_machine_readable = has_machine_readable;
                v.yaml_content = new_content;
                v.content_sha = None;
            }
        }
        true
    }

    /// Get the number of loaded laws.
    pub fn len(&self) -> usize {
        self.laws.len()
    }

    /// Check if the source map is empty.
    pub fn is_empty(&self) -> bool {
        self.laws.is_empty()
    }

    /// Get all conflict resolutions that occurred during loading.
    pub fn resolved_conflicts(&self) -> &[ConflictResolution] {
        &self.resolved_conflicts
    }
}

/// Extract a YYYY-MM-DD date from the filename component of a path.
///
/// Matches the convention `…/law_id/2025-01-01.yaml`. The single extractor for
/// every route that feeds [`pick_best_version`] — local scan and GitHub tree
/// alike. That comparison is lexicographic, so any stem that is not a date
/// sorts against real dates by accident: `2025-01-01-concept` is greater than
/// `2025-01-01` and would win. Returning `None` keeps such a file out of the
/// running instead.
pub fn extract_date_from_path(path: &str) -> Option<String> {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let stem = filename.strip_suffix(".yaml")?;
    let bytes = stem.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !(0..10).all(|i| i == 4 || i == 7 || bytes[i].is_ascii_digit()) {
        return None;
    }
    let month: u32 = stem.get(5..7)?.parse().ok()?;
    let day: u32 = stem.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(stem.to_string())
}

/// Decide whether `new_date` should replace `existing_date`.
///
/// Rules:
/// 1. Currently valid (date <= today) beats future-only.
/// 2. Among currently valid dates, the latest wins (most up-to-date).
/// 3. Among future dates, the latest wins.
pub(crate) fn pick_best_version(existing: Option<&str>, new: Option<&str>, today: &str) -> bool {
    match (existing, new) {
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (None, None) => false,
        (Some(e), Some(n)) => {
            let e_valid = e <= today;
            let n_valid = n <= today;
            match (e_valid, n_valid) {
                // Both valid or both future → latest date wins
                _ if e_valid == n_valid => n > e,
                // Only new is valid now → new wins
                (false, true) => true,
                // Only existing is valid now → existing stays
                (true, false) => false,
                _ => unreachable!(),
            }
        }
    }
}

/// Verify that a string parses as well-formed YAML without enforcing a
/// particular schema. Used by write handlers that want to reject garbage
/// input before persisting to the corpus backend, without committing to
/// the corpus library's full law-schema validation (which belongs in a
/// separate layer).
pub fn validate_yaml_syntax(content: &str) -> Result<()> {
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content)?;
    Ok(())
}

/// Extract the top-level `$id` field from a YAML string.
///
/// Delegates to the shared tolerant header parser in `regelrecht-law-model`,
/// which matches `$id:` only at the start of a line (so a nested `$id:` is not
/// picked up) and tolerates a malformed document body.
pub fn extract_law_id(yaml: &str) -> Option<String> {
    regelrecht_law_model::parse_law_header(yaml).id
}

/// Extract the top-level literal `name` field from a YAML string.
///
/// Skips names starting with `#` (output references resolved at runtime).
fn extract_law_name(yaml: &str) -> Option<String> {
    regelrecht_law_model::parse_law_header(yaml)
        .name
        .filter(|name| !name.starts_with('#'))
}

/// Extract the raw `name:` value including `#` references.
fn extract_raw_name(yaml: &str) -> Option<String> {
    regelrecht_law_model::parse_law_header(yaml).name
}

// --- Minimal, deliberately tolerant view types for display-name and output
// resolution. These are NOT the canonical law model (`regelrecht-law-model`):
// they intentionally make every field optional so they keep working on editor
// drafts and mid-enrichment files that the strict model would reject. ---

#[derive(Deserialize, Default)]
struct LawDoc {
    /// Top-level `name:` — a literal (`Kieswet`) or an output reference
    /// (`#wet_naam`). Read straight off the shared parse so the loaded-field
    /// derivation doesn't re-scan the raw string.
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    articles: Vec<LawArticle>,
}

#[derive(Deserialize, Default)]
struct LawArticle {
    #[serde(default)]
    number: Option<String>,
    #[serde(default)]
    machine_readable: Option<LawMr>,
}

#[derive(Deserialize, Default)]
struct LawMr {
    #[serde(default)]
    execution: Option<LawExec>,
    #[serde(default)]
    implements: Vec<LawImplements>,
}

/// One `machine_readable.implements` entry — the IoC link from a lower
/// regulation to the higher law whose `open_term` it fills. Only `law`
/// (the higher law's `$id`) is needed to build the reverse "who implements
/// me" lookup.
#[derive(Deserialize, Default)]
struct LawImplements {
    #[serde(default)]
    law: String,
}

#[derive(Deserialize, Default)]
struct LawExec {
    #[serde(default)]
    actions: Vec<LawAction>,
    #[serde(default)]
    output: Vec<LawOutput>,
    #[serde(default)]
    parameters: Vec<LawParam>,
}

/// A parameter entry from `execution.parameters`.
#[derive(Deserialize, Default, Clone)]
struct LawParam {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    param_type: Option<String>,
}

#[derive(Deserialize, Default)]
struct LawAction {
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    value: Option<serde_yaml_ng::Value>,
}

/// An output entry from `execution.output`.
#[derive(Deserialize, Default)]
struct LawOutput {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    output_type: Option<String>,
}

/// Resolve a law's human-readable display name.
///
/// If the YAML has a literal `name:` field (e.g. `name: Kieswet`), returns
/// that. If the name is an output reference (e.g. `name: '#wet_naam'`),
/// parses the YAML to find the action whose output matches the reference
/// and returns its scalar value. Returns `None` when no name can be resolved.
pub fn resolve_display_name(yaml: &str) -> Option<String> {
    // Fast path: literal name
    if let Some(name) = extract_law_name(yaml) {
        return Some(name);
    }

    // Check for # reference
    let raw = extract_raw_name(yaml)?;
    let reference = raw.strip_prefix('#')?;

    // Parse YAML to find the action that resolves this reference
    let doc: LawDoc = serde_yaml_ng::from_str(yaml).ok()?;
    resolve_name_reference(&doc, reference)
}

/// Find the action whose `output` matches `reference` and return its
/// scalar string value — the resolution behind `name: '#output_ref'`.
fn resolve_name_reference(doc: &LawDoc, reference: &str) -> Option<String> {
    for article in &doc.articles {
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        let Some(exec) = &mr.execution else {
            continue;
        };
        for action in &exec.actions {
            if action.output.as_deref() == Some(reference) {
                if let Some(serde_yaml_ng::Value::String(s)) = &action.value {
                    return Some(s.clone());
                }
            }
        }
    }

    None
}

/// Compute the content-derived [`LoadedLaw`] fields (`display_name`,
/// `implements`, `has_machine_readable`) with a single YAML parse. Called once
/// per law at load / update time so the read paths never have to re-parse
/// bodies per request. Returns `(name, vec![], false)` for unparseable content,
/// the same degraded result the per-request resolvers used to produce.
fn derive_loaded_fields(yaml: &str) -> (Option<String>, Vec<String>, bool) {
    // `implements` always needs the parsed doc; the display name only
    // does for `name: '#ref'` laws, but sharing one parse keeps this a
    // single pass either way.
    let doc: LawDoc = match serde_yaml_ng::from_str(yaml) {
        Ok(d) => d,
        Err(_) => return (extract_law_name(yaml), Vec::new(), false),
    };
    // Resolve the display name from the already-parsed `name` field rather
    // than re-scanning the raw string: a literal name is used as-is, a
    // `#ref` is resolved against this doc's actions.
    let display_name = doc.name.as_deref().and_then(|raw| {
        let raw = raw.trim();
        match raw.strip_prefix('#') {
            Some(reference) => resolve_name_reference(&doc, reference),
            None if !raw.is_empty() => Some(raw.to_string()),
            None => None,
        }
    });
    let has_machine_readable = doc.articles.iter().any(|a| a.machine_readable.is_some());
    (
        display_name,
        collect_implements_from_doc(&doc),
        has_machine_readable,
    )
}

/// A collected output with the parameters required by its article's execution block.
pub struct CollectedOutput {
    pub name: String,
    pub output_type: String,
    pub article_number: String,
    /// Parameters declared on the execution block that contains this output.
    /// The caller needs to supply these when referencing this output as a source.
    pub parameters: Vec<(String, String)>, // (name, type)
}

/// Collect all outputs declared across all articles in a law.
///
/// Each output includes the parameters required by its article's execution
/// block, so the UI can pre-populate source.parameters when the user selects
/// an output.
pub fn collect_law_outputs(yaml: &str) -> Vec<CollectedOutput> {
    let doc: LawDoc = match serde_yaml_ng::from_str(yaml) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for article in &doc.articles {
        let article_number = article.number.as_deref().unwrap_or("").to_string();
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        let Some(exec) = &mr.execution else {
            continue;
        };
        let params: Vec<(String, String)> = exec
            .parameters
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    p.param_type.clone().unwrap_or_else(|| "string".to_string()),
                )
            })
            .collect();

        for output in &exec.output {
            if !output.name.is_empty() && !seen.contains(&output.name) {
                seen.insert(output.name.clone());
                results.push(CollectedOutput {
                    name: output.name.clone(),
                    output_type: output
                        .output_type
                        .clone()
                        .unwrap_or_else(|| "string".to_string()),
                    article_number: article_number.clone(),
                    parameters: params.clone(),
                });
            }
        }
    }

    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

/// Collect the `$id`s of the higher laws that this law's articles declare
/// they `implements` (the IoC reverse link: a lower regulation filling a
/// higher law's `open_term`). Each referenced law id appears at most once.
///
/// Used to answer "which laws implement law X" server-side, so the editor's
/// scenario dependency loader doesn't have to fetch and parse every law in
/// the corpus over HTTP just to find the handful of implementing regulations.
pub fn collect_law_implements(yaml: &str) -> Vec<String> {
    try_collect_law_implements(yaml).unwrap_or_default()
}

/// Fallible variant of [`collect_law_implements`]: returns `Err` exactly
/// when the tolerant law parse fails — the case the infallible variant
/// silently degrades to "implements nothing".
///
/// The implements-index *generator* must use this one: recording a parse
/// failure as an empty list in a committed index artefact would make the
/// failure permanent and invisible (the consumer trusts the index and never
/// re-parses the body), so the generator counts these and refuses to emit
/// an index. Read paths that merely degrade per-request keep using the
/// infallible variant.
pub fn try_collect_law_implements(
    yaml: &str,
) -> std::result::Result<Vec<String>, serde_yaml_ng::Error> {
    let doc: LawDoc = serde_yaml_ng::from_str(yaml)?;
    Ok(collect_implements_from_doc(&doc))
}

/// Doc-based core of [`collect_law_implements`], shared with the
/// parse-once load path ([`derive_loaded_fields`]).
fn collect_implements_from_doc(doc: &LawDoc) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for article in &doc.articles {
        let Some(mr) = &article.machine_readable else {
            continue;
        };
        for decl in &mr.implements {
            if !decl.law.is_empty() && seen.insert(decl.law.clone()) {
                out.push(decl.law.clone());
            }
        }
    }
    out
}

/// How one law points at another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LawReferenceKind {
    /// An input field resolved from another law's output
    /// (`source: {regulation: …, output: …}`).
    SourceRegulation,
    /// An input field that calls the decision of an article of another law
    /// (`source: {article: "<regulation>#<article>"}`, schema v0.8.4). Only a
    /// fixed target: a `$`-target names its article at run time.
    SourceArticle,
    /// The IoC link from a lower regulation to the higher law whose
    /// `open_term` it fills (`machine_readable.implements[].law`).
    Implements,
}

/// One `$id` a law body points at, with the kind of link it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawReference {
    pub law_id: String,
    pub kind: LawReferenceKind,
}

/// Collect every other law `$id` this body points at: the `source.regulation`
/// targets of its input fields, the regulation of a fixed `source.article`
/// target, plus its `implements` declarations. Each
/// `(kind, id)` pair appears at most once; order of first occurrence is kept.
///
/// Deliberately a **structural walk** over the parsed document rather than a
/// path-pinned lookup: this feeds the integrity report, whose whole job is to
/// find dangling references, so it must not silently miss one that sits at a
/// nesting the schema grows later. A body that does not parse yields no
/// references (the caller reports the parse failure separately) — same
/// tolerance as [`collect_law_implements`].
pub fn collect_law_references(yaml: &str) -> Vec<LawReference> {
    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(yaml) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    walk_law_references(&value, &mut seen, &mut out);
    out
}

/// Record one reference, unless the same `(kind, id)` was already seen.
fn push_law_reference(
    kind: LawReferenceKind,
    id: &str,
    seen: &mut HashSet<(LawReferenceKind, String)>,
    out: &mut Vec<LawReference>,
) {
    if !id.is_empty() && seen.insert((kind, id.to_string())) {
        out.push(LawReference {
            law_id: id.to_string(),
            kind,
        });
    }
}

/// Recursive half of [`collect_law_references`].
fn walk_law_references(
    value: &serde_yaml_ng::Value,
    seen: &mut HashSet<(LawReferenceKind, String)>,
    out: &mut Vec<LawReference>,
) {
    match value {
        serde_yaml_ng::Value::Mapping(map) => {
            for (key, child) in map {
                match key.as_str() {
                    // `source: {regulation: "other_law", output: …}`
                    Some("source") => {
                        if let Some(id) = child.get("regulation").and_then(|v| v.as_str()) {
                            push_law_reference(LawReferenceKind::SourceRegulation, id, seen, out);
                        }
                        // `source: {article: "other_law#3"}`; a `$`-target is
                        // known only at run time.
                        if let Some((id, _)) = child
                            .get("article")
                            .and_then(|v| v.as_str())
                            .filter(|target| !target.starts_with('$'))
                            .and_then(|target| target.split_once('#'))
                        {
                            push_law_reference(LawReferenceKind::SourceArticle, id, seen, out);
                        }
                    }
                    // `implements: [{law: "higher_law", …}]`
                    Some("implements") => {
                        if let Some(items) = child.as_sequence() {
                            for item in items {
                                if let Some(id) = item.get("law").and_then(|v| v.as_str()) {
                                    push_law_reference(LawReferenceKind::Implements, id, seen, out);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Recurse after the direct matches so `out` follows the document
            // order rather than the recursion's.
            for (_, child) in map {
                walk_law_references(child, seen, out);
            }
        }
        serde_yaml_ng::Value::Sequence(items) => {
            for item in items {
                walk_law_references(item, seen, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::{LocalSource, SourceType};
    use std::fs;
    use tempfile::TempDir;

    fn make_source(id: &str, name: &str, path: &Path, priority: u32) -> Source {
        Source {
            id: id.to_string(),
            name: name.to_string(),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: path.to_path_buf(),
                },
            },
            scopes: vec![],
            priority,
            auth_ref: None,
            strict_auth: false,
            enriched: None,
            load_all: false,
        }
    }

    fn write_yaml(dir: &Path, subpath: &str, id: &str) {
        let path = dir.join(subpath);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                "$id: {id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles: []\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_extract_law_id() {
        assert_eq!(
            extract_law_id("$id: my_law\nfoo: bar"),
            Some("my_law".to_string())
        );
        assert_eq!(
            extract_law_id("$id: \"quoted_id\"\nfoo: bar"),
            Some("quoted_id".to_string())
        );
        assert_eq!(extract_law_id("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_extract_law_id_ignores_indented() {
        // Nested $id: should not be matched
        let yaml = "name: test\narticles:\n  - $id: nested_id\n";
        assert_eq!(extract_law_id(yaml), None);

        // But top-level $id: should still work
        let yaml = "$id: top_level\narticles:\n  - $id: nested_id\n";
        assert_eq!(extract_law_id(yaml), Some("top_level".to_string()));
    }

    #[test]
    fn test_update_yaml_content_updates_existing_law() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "wet/test_wet/2025-01-01.yaml", "test_wet");

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let original = map.get_law("test_wet").unwrap().clone();

        let new_content =
            "$id: test_wet\nname: Updated Name\nregulatory_layer: WET\narticles: []\n".to_string();
        let updated = map.update_yaml_content("test_wet", new_content.clone());

        assert!(updated, "update should report success for existing law");
        let law = map.get_law("test_wet").unwrap();
        assert_eq!(law.yaml_content, new_content);
        assert_eq!(law.name.as_deref(), Some("Updated Name"));
        // All provenance fields are preserved — update_yaml_content only
        // touches content + name. `relative_path` is load-bearing because
        // the editor-api write handler targets it; a regression that
        // cleared it would silently send reads and writes to different
        // files, so we assert it explicitly alongside file_path/source_*.
        assert_eq!(law.file_path, original.file_path);
        assert_eq!(law.relative_path, original.relative_path);
        assert_eq!(law.source_id, original.source_id);
        assert_eq!(law.source_name, original.source_name);
        assert_eq!(law.source_priority, original.source_priority);
    }

    #[test]
    fn test_update_yaml_content_recomputes_name_to_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("wet/test_wet/2025-01-01.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "$id: test_wet\nname: Original\nregulatory_layer: WET\narticles: []\n",
        )
        .unwrap();

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();
        assert_eq!(
            map.get_law("test_wet").unwrap().name.as_deref(),
            Some("Original")
        );

        // Remove the `name:` field — name should recompute to None.
        let new_content = "$id: test_wet\nregulatory_layer: WET\narticles: []\n".to_string();
        let updated = map.update_yaml_content("test_wet", new_content);
        assert!(updated);
        assert_eq!(map.get_law("test_wet").unwrap().name, None);
    }

    #[test]
    fn test_update_yaml_content_missing_law_returns_false() {
        let mut map = SourceMap::new("2026-06-01");
        let updated = map.update_yaml_content("nonexistent_law", "$id: foo\n".to_string());
        assert!(!updated, "update should report failure for missing law");
        assert_eq!(map.len(), 0, "missing law should not be inserted");
    }

    #[test]
    fn test_validate_yaml_syntax_accepts_well_formed() {
        assert!(validate_yaml_syntax("$id: foo\nname: bar\narticles: []\n").is_ok());
        assert!(validate_yaml_syntax("---\nfoo: 1\n").is_ok());
        assert!(validate_yaml_syntax("").is_ok()); // empty doc is valid
    }

    #[test]
    fn test_validate_yaml_syntax_rejects_garbage() {
        // Unclosed quote.
        assert!(validate_yaml_syntax("name: \"unterminated\nfoo: bar\n").is_err());
        // Tab indentation inside a block mapping.
        assert!(validate_yaml_syntax("name:\n\tfoo: bar\n").is_err());
    }

    #[test]
    fn test_load_single_source() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "wet/test_wet/2025-01-01.yaml", "test_wet");

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 1);
        assert_eq!(map.len(), 1);

        let law = map.get_law("test_wet").unwrap();
        assert_eq!(law.source_id, "central");
        assert_eq!(law.source_priority, 1);
        assert!(law.is_loaded(), "a content-loaded law reports loaded");
    }

    #[test]
    fn test_metadata_entry_is_unloaded_with_stripped_path() {
        let mut map = SourceMap::new("2026-06-01");
        // `file_path` is the in-repo path; the source is rooted at
        // `regulation/nl`, so the stored relative_path drops that prefix.
        map.load_metadata_entry(
            "besluit_zorgverzekering_example",
            "regulation/nl/amvb/besluit_zorgverzekering_example/2024-01-01.yaml",
            Some("regulation/nl"),
            "traject-own-abc",
            "example-org/regelrecht-corpus-example",
            0,
            Some("blob-sha-1"),
        )
        .unwrap();

        let law = map.get_law("besluit_zorgverzekering_example").unwrap();
        assert!(!law.is_loaded(), "metadata-only entry has no body yet");
        assert_eq!(law.yaml_content, "");
        assert_eq!(law.name, None);
        assert_eq!(
            law.relative_path,
            "amvb/besluit_zorgverzekering_example/2024-01-01.yaml"
        );
        assert_eq!(law.source_id, "traject-own-abc");
        assert_eq!(law.source_priority, 0);
        assert_eq!(law.content_sha.as_deref(), Some("blob-sha-1"));
    }

    #[test]
    fn test_multi_source_no_overlap() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/law_a/2025.yaml", "law_a");
        write_yaml(dir_b.path(), "wet/law_b/2025.yaml", "law_b");

        let source_a = make_source("central", "Central", dir_a.path(), 1);
        let source_b = make_source("gemeente", "Gemeente", dir_b.path(), 10);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source_a).unwrap();
        map.load_source(&source_b).unwrap();

        assert_eq!(map.len(), 2);
        assert_eq!(map.get_law("law_a").unwrap().source_id, "central");
        assert_eq!(map.get_law("law_b").unwrap().source_id, "gemeente");
    }

    #[test]
    fn test_priority_conflict_lower_wins() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/shared/2025.yaml", "shared_law");
        write_yaml(dir_b.path(), "wet/shared/2025.yaml", "shared_law");

        let source_a = make_source("central", "Central", dir_a.path(), 1);
        let source_b = make_source("overlap", "Overlap", dir_b.path(), 10);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source_a).unwrap();
        map.load_source(&source_b).unwrap();

        assert_eq!(map.len(), 1);
        let law = map.get_law("shared_law").unwrap();
        assert_eq!(law.source_id, "central"); // Priority 1 wins over 10

        assert_eq!(map.resolved_conflicts().len(), 1);
        assert_eq!(map.resolved_conflicts()[0].winner_source_id, "central");
    }

    #[test]
    fn test_equal_priority_different_sources_is_error() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/dup/2025.yaml", "dup_law");
        write_yaml(dir_b.path(), "wet/dup/2025.yaml", "dup_law");

        let source_a = make_source("source-a", "Source A", dir_a.path(), 5);
        let source_b = make_source("source-b", "Source B", dir_b.path(), 5);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source_a).unwrap();
        let result = map.load_source(&source_b);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("dup_law"));
        assert!(err.contains("equal priority"));
    }

    #[test]
    fn test_same_source_multiple_versions_keeps_latest_valid() {
        let dir = TempDir::new().unwrap();

        // Two versions of the same law, both currently valid (dates in the past)
        write_yaml(dir.path(), "wet/my_law/2024-01-01.yaml", "my_law");
        write_yaml(dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");

        let source = make_source("local", "Local", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        let count = map.load_source(&source).unwrap();

        // Both files are loaded but only one law in the map
        assert_eq!(count, 2);
        assert_eq!(map.len(), 1);

        let law = map.get_law("my_law").unwrap();
        // 2025 version should win (latest valid)
        assert!(law.file_path.contains("2025-01-01"));
    }

    #[test]
    fn test_same_source_valid_beats_future() {
        let dir = TempDir::new().unwrap();

        // One valid now, one far in the future
        write_yaml(dir.path(), "wet/my_law/2024-01-01.yaml", "my_law");
        write_yaml(dir.path(), "wet/my_law/2099-01-01.yaml", "my_law");

        let source = make_source("local", "Local", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let law = map.get_law("my_law").unwrap();
        // 2024 is currently valid, 2099 is future → 2024 wins
        assert!(law.file_path.contains("2024-01-01"));
    }

    #[test]
    fn test_get_law_versions_keeps_all_versions_same_source() {
        let dir = TempDir::new().unwrap();

        write_yaml(dir.path(), "wet/my_law/2024-01-01.yaml", "my_law");
        write_yaml(dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");

        let source = make_source("local", "Local", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        // `get_law` still collapses to one (the today's-best entry)...
        assert_eq!(map.len(), 1);
        // ...but `get_law_versions` exposes the full set, newest-first.
        let versions = map.get_law_versions("my_law").unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions[0].file_path.contains("2025-01-01"));
        assert!(versions[1].file_path.contains("2024-01-01"));
        assert!(map.get_law_versions("nonexistent").is_none());
    }

    #[test]
    fn test_get_law_versions_unions_across_sources_keeping_future() {
        // The federation bug shape: a higher-priority source has only an
        // in-force version, a lower-priority source additionally has a
        // future-dated one. `get_law` returns the priority winner (the
        // in-force local copy), but the engine must *also* see the future
        // version so a scenario as-of a later date can resolve it — and the
        // future version must NOT shadow the in-force one for earlier dates.
        let local_dir = TempDir::new().unwrap();
        let central_dir = TempDir::new().unwrap();

        write_yaml(local_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2099-01-01.yaml", "my_law");

        let local = make_source("local", "Local", local_dir.path(), 1);
        let central = make_source("central", "Central", central_dir.path(), 2);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&local).unwrap();
        map.load_source(&central).unwrap();

        // Single-best collapse: local (priority 1) wins the contested 2025 date.
        assert_eq!(map.get_law("my_law").unwrap().source_id, "local");

        // Version set: the union across sources — one entry per date, with the
        // 2025 date resolved to the higher-priority (local) body.
        let versions = map.get_law_versions("my_law").unwrap();
        assert_eq!(versions.len(), 2, "expected the 2025 + 2099 union");
        let v2025 = versions
            .iter()
            .find(|v| v.file_path.contains("2025-01-01"))
            .unwrap();
        assert_eq!(v2025.source_id, "local", "2025 date won by priority");
        assert!(
            versions.iter().any(|v| v.file_path.contains("2099-01-01")),
            "central's future version must be kept, not dropped"
        );
    }

    #[test]
    fn test_get_law_versions_priority_order_independent() {
        // Mirrors `..._unions_across_sources_keeping_future` (local 2025 +
        // central 2025 + central 2099) but indexes central FIRST, so this
        // exercises both order-independent behaviours at once: the contested
        // 2025 date still resolves to the higher-priority local body, and
        // central's future 2099 version is still kept in the union.
        let local_dir = TempDir::new().unwrap();
        let central_dir = TempDir::new().unwrap();
        write_yaml(local_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2099-01-01.yaml", "my_law");

        let local = make_source("local", "Local", local_dir.path(), 1);
        let central = make_source("central", "Central", central_dir.path(), 2);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&central).unwrap();
        map.load_source(&local).unwrap();

        let versions = map.get_law_versions("my_law").unwrap();
        assert_eq!(versions.len(), 2, "expected the 2025 + 2099 union");
        let v2025 = versions
            .iter()
            .find(|v| v.file_path.contains("2025-01-01"))
            .unwrap();
        assert_eq!(v2025.source_id, "local", "2025 date won by priority");
        assert!(
            versions.iter().any(|v| v.file_path.contains("2099-01-01")),
            "central's future version must be kept regardless of index order"
        );
    }

    #[test]
    fn test_version_in_force_beats_priority_across_sources() {
        // An example source (priority 1) holds an older version; the central
        // corpus (priority 2) a newer one. By date the newer one is the law.
        let example_dir = TempDir::new().unwrap();
        let central_dir = TempDir::new().unwrap();
        write_yaml(example_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2026-01-01.yaml", "my_law");

        let example = make_source("example", "Example", example_dir.path(), 1);
        let central = make_source("central", "Central", central_dir.path(), 2);

        // Priority first (the default): the example's older version.
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&example).unwrap();
        map.load_source(&central).unwrap();
        assert_eq!(map.get_law("my_law").unwrap().source_id, "example");

        // Version in force first: central's newer version, the older one kept.
        map.prefer_version_in_force();
        let law = map.get_law("my_law").unwrap();
        assert_eq!(law.source_id, "central");
        assert!(law.file_path.contains("2026-01-01"));
        assert_eq!(map.get_law_versions("my_law").unwrap().len(), 2);
    }

    #[test]
    fn test_version_in_force_keeps_priority_for_the_same_date() {
        // A local override of the same version still wins.
        let local_dir = TempDir::new().unwrap();
        let central_dir = TempDir::new().unwrap();
        write_yaml(local_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");

        let local = make_source("local", "Local", local_dir.path(), 1);
        let central = make_source("central", "Central", central_dir.path(), 2);

        let mut map = SourceMap::new("2026-06-01");
        map.prefer_version_in_force();
        map.load_source(&central).unwrap();
        map.load_source(&local).unwrap();

        assert_eq!(map.get_law("my_law").unwrap().source_id, "local");
        assert_eq!(map.resolved_conflicts().len(), 1);
    }

    #[test]
    fn test_version_in_force_ignores_a_future_version() {
        // A version that is not in force yet does not become current.
        let local_dir = TempDir::new().unwrap();
        let central_dir = TempDir::new().unwrap();
        write_yaml(local_dir.path(), "wet/my_law/2025-01-01.yaml", "my_law");
        write_yaml(central_dir.path(), "wet/my_law/2099-01-01.yaml", "my_law");

        let local = make_source("local", "Local", local_dir.path(), 2);
        let central = make_source("central", "Central", central_dir.path(), 1);

        let mut map = SourceMap::new("2026-06-01");
        map.prefer_version_in_force();
        map.load_source(&central).unwrap();
        map.load_source(&local).unwrap();

        assert_eq!(map.get_law("my_law").unwrap().source_id, "local");
    }

    #[test]
    fn test_version_in_force_still_rejects_equal_priority() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        write_yaml(dir_a.path(), "wet/dup/2025-01-01.yaml", "dup_law");
        write_yaml(dir_b.path(), "wet/dup/2026-01-01.yaml", "dup_law");

        let source_a = make_source("source-a", "Source A", dir_a.path(), 5);
        let source_b = make_source("source-b", "Source B", dir_b.path(), 5);

        let mut map = SourceMap::new("2026-06-01");
        map.prefer_version_in_force();
        map.load_source(&source_a).unwrap();
        let err = map.load_source(&source_b).unwrap_err().to_string();
        assert!(err.contains("equal priority"));
    }

    #[test]
    fn test_empty_directory() {
        let dir = TempDir::new().unwrap();
        let source = make_source("empty", "Empty", dir.path(), 1);

        let mut map = SourceMap::new("2026-06-01");
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 0);
        assert!(map.is_empty());
    }

    #[test]
    fn test_nonexistent_directory() {
        let source = make_source("missing", "Missing", Path::new("/nonexistent"), 1);

        let mut map = SourceMap::new("2026-06-01");
        let count = map.load_source(&source).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_reverse_load_order_still_priority_wins() {
        // Load high-priority source second — it should still win
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        write_yaml(dir_a.path(), "wet/law/2025.yaml", "contested_law");
        write_yaml(dir_b.path(), "wet/law/2025.yaml", "contested_law");

        let source_low = make_source("low", "Low Priority", dir_a.path(), 100);
        let source_high = make_source("high", "High Priority", dir_b.path(), 1);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source_low).unwrap();
        map.load_source(&source_high).unwrap();

        assert_eq!(map.get_law("contested_law").unwrap().source_id, "high");
    }

    #[test]
    fn test_resolve_display_name_literal() {
        let yaml = "$id: kieswet\nname: Kieswet\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), Some("Kieswet".to_string()));
    }

    #[test]
    fn test_resolve_display_name_output_reference() {
        let yaml = r#"$id: wet_op_de_zorgtoeslag
name: '#wet_naam'
articles:
  - number: '8'
    machine_readable:
      execution:
        actions:
          - output: wet_naam
            value: Wet op de zorgtoeslag
"#;
        assert_eq!(
            resolve_display_name(yaml),
            Some("Wet op de zorgtoeslag".to_string())
        );
    }

    #[test]
    fn test_resolve_display_name_unresolvable_reference() {
        let yaml = "$id: test\nname: '#missing_output'\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), None);
    }

    #[test]
    fn test_resolve_display_name_no_name_field() {
        let yaml = "$id: test\narticles: []\n";
        assert_eq!(resolve_display_name(yaml), None);
    }

    /// A law body with a `name: '#ref'` display name and an `implements`
    /// declaration, for the precompute tests below.
    const DERIVED_FIELDS_YAML: &str = r#"$id: regeling_zorgtoeslag
name: '#regeling_naam'
articles:
  - number: '1'
    machine_readable:
      execution:
        actions:
          - output: regeling_naam
            value: Regeling zorgtoeslag
      implements:
        - law: wet_op_de_zorgtoeslag
          open_term: standaardpremie
"#;

    #[test]
    fn test_load_precomputes_display_name_and_implements() {
        // The list/implementors endpoints read these fields straight off
        // the loaded law instead of re-parsing every body per request, so
        // load time MUST populate them.
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join("regeling/regeling_zorgtoeslag/2025-01-01.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, DERIVED_FIELDS_YAML).unwrap();

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let law = map.get_law("regeling_zorgtoeslag").unwrap();
        // `name` only carries literal names; the resolved `#ref` lands in
        // `display_name`.
        assert_eq!(law.name, None);
        assert_eq!(law.display_name.as_deref(), Some("Regeling zorgtoeslag"));
        assert_eq!(law.implements, vec!["wet_op_de_zorgtoeslag".to_string()]);
    }

    #[test]
    fn test_load_fetched_file_precomputes_display_name_and_implements() {
        let mut map = SourceMap::new("2026-06-01");
        map.load_fetched_file(
            DERIVED_FIELDS_YAML,
            "regulation/nl/regeling/regeling_zorgtoeslag/2025-01-01.yaml",
            Some("regulation/nl"),
            "central",
            "Central",
            1,
        )
        .unwrap();

        let law = map.get_law("regeling_zorgtoeslag").unwrap();
        assert_eq!(law.display_name.as_deref(), Some("Regeling zorgtoeslag"));
        assert_eq!(law.implements, vec!["wet_op_de_zorgtoeslag".to_string()]);
    }

    #[test]
    fn test_load_literal_name_display_name_matches_name() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("wet/kieswet/2025-01-01.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "$id: kieswet\nname: Kieswet\narticles: []\n").unwrap();

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let law = map.get_law("kieswet").unwrap();
        assert_eq!(law.name.as_deref(), Some("Kieswet"));
        assert_eq!(law.display_name.as_deref(), Some("Kieswet"));
        assert!(law.implements.is_empty());
    }

    #[test]
    fn test_metadata_entry_has_no_derived_fields() {
        // Metadata-only entries have no body to derive from — the fields
        // stay empty until the body is fetched and (in the editor flow)
        // a snapshot rebuild or `update_yaml_content` recomputes them.
        let mut map = SourceMap::new("2026-06-01");
        map.load_metadata_entry(
            "some_wet",
            "regulation/nl/wet/some_wet/2025-01-01.yaml",
            Some("regulation/nl"),
            "central",
            "Central",
            1,
            None,
        )
        .unwrap();
        let law = map.get_law("some_wet").unwrap();
        assert_eq!(law.display_name, None);
        assert!(law.implements.is_empty());
    }

    #[test]
    fn test_update_yaml_content_recomputes_derived_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join("regeling/regeling_zorgtoeslag/2025-01-01.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, DERIVED_FIELDS_YAML).unwrap();

        let source = make_source("central", "Central", dir.path(), 1);
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        // Edit drops the implements block and switches to a literal name:
        // both derived fields must follow the new content.
        let new_content =
            "$id: regeling_zorgtoeslag\nname: Nieuwe naam\narticles: []\n".to_string();
        assert!(map.update_yaml_content("regeling_zorgtoeslag", new_content));

        let law = map.get_law("regeling_zorgtoeslag").unwrap();
        assert_eq!(law.display_name.as_deref(), Some("Nieuwe naam"));
        assert!(law.implements.is_empty());
    }

    #[test]
    fn test_collect_law_outputs() {
        let yaml = r#"$id: brp
articles:
  - number: '2.7'
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
          - name: peildatum
            type: date
        output:
          - name: leeftijd
            type: number
        actions: []
  - number: '2.8'
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        output:
          - name: heeft_partner
            type: boolean
        actions: []
"#;
        let outputs = collect_law_outputs(yaml);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].name, "heeft_partner");
        assert_eq!(outputs[0].output_type, "boolean");
        assert_eq!(outputs[0].article_number, "2.8");
        assert_eq!(
            outputs[0].parameters,
            vec![("bsn".to_string(), "string".to_string())]
        );

        assert_eq!(outputs[1].name, "leeftijd");
        assert_eq!(outputs[1].output_type, "number");
        assert_eq!(outputs[1].article_number, "2.7");
        assert_eq!(
            outputs[1].parameters,
            vec![
                ("bsn".to_string(), "string".to_string()),
                ("peildatum".to_string(), "date".to_string()),
            ]
        );
    }

    #[test]
    fn test_collect_law_outputs_deduplicates() {
        let yaml = r#"$id: test
articles:
  - number: '1'
    machine_readable:
      execution:
        output:
          - name: foo
            type: string
        actions: []
  - number: '2'
    machine_readable:
      execution:
        output:
          - name: foo
            type: number
        actions: []
"#;
        let outputs = collect_law_outputs(yaml);
        assert_eq!(outputs.len(), 1);
        // First occurrence wins
        assert_eq!(outputs[0].output_type, "string");
    }

    #[test]
    fn test_collect_law_outputs_empty() {
        let yaml = "$id: test\narticles: []\n";
        let outputs = collect_law_outputs(yaml);
        assert!(outputs.is_empty());
    }

    #[test]
    fn test_collect_law_implements() {
        let yaml = "\
$id: regeling_standaardpremie
articles:
  - number: '1'
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
        - law: wet_op_de_zorgtoeslag
          article: '5'
          open_term: iets_anders
  - number: '2'
    machine_readable:
      implements:
        - law: zorgverzekeringswet
          article: '1'
          open_term: dekking
";
        // Deduplicated across articles, preserves first-seen order.
        assert_eq!(
            collect_law_implements(yaml),
            vec![
                "wet_op_de_zorgtoeslag".to_string(),
                "zorgverzekeringswet".to_string()
            ]
        );
    }

    #[test]
    fn test_collect_law_implements_none() {
        let yaml = "$id: x\narticles:\n  - number: '1'\n    machine_readable:\n      execution:\n        output: []\n";
        assert!(collect_law_implements(yaml).is_empty());
        // Malformed YAML is tolerated as "no implements".
        assert!(collect_law_implements("not: [valid").is_empty());
    }

    #[test]
    fn test_try_collect_law_implements_distinguishes_parse_failure_from_empty() {
        // A law that genuinely implements nothing: Ok(empty).
        let empty = try_collect_law_implements("$id: x\narticles: []\n").unwrap();
        assert!(empty.is_empty());

        // Malformed YAML: Err — the index generator must fail on this
        // instead of committing "implements nothing".
        assert!(try_collect_law_implements("not: [valid").is_err());
    }

    #[test]
    fn test_collect_law_references_picks_up_both_link_kinds() {
        let yaml = "\
$id: wet_a
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: inkomen
            type: amount
            source:
              regulation: wet_b
              output: toetsingsinkomen
          - name: verzekerd
            type: boolean
            source:
              regulation: wet_c
              output: is_verzekerd
      implements:
        - law: wet_d
          article: '4'
          open_term: tarief
";
        assert_eq!(
            collect_law_references(yaml),
            vec![
                LawReference {
                    law_id: "wet_d".to_string(),
                    kind: LawReferenceKind::Implements,
                },
                LawReference {
                    law_id: "wet_b".to_string(),
                    kind: LawReferenceKind::SourceRegulation,
                },
                LawReference {
                    law_id: "wet_c".to_string(),
                    kind: LawReferenceKind::SourceRegulation,
                },
            ]
        );
    }

    #[test]
    fn test_collect_law_references_picks_up_a_fixed_article_target() {
        let yaml = "\
$id: wet_a
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: vast
            type: decision
            source:
              article: wet_b#3
          - name: uit_data
            type: decision
            source:
              article: $besluit.basis
";
        assert_eq!(
            collect_law_references(yaml),
            vec![LawReference {
                law_id: "wet_b".to_string(),
                kind: LawReferenceKind::SourceArticle,
            }]
        );
    }

    #[test]
    fn test_collect_law_references_dedupes_and_tolerates_garbage() {
        let yaml = "\
$id: wet_a
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: een
            source:
              regulation: wet_b
              output: x
          - name: twee
            source:
              regulation: wet_b
              output: y
";
        assert_eq!(collect_law_references(yaml).len(), 1);
        assert!(collect_law_references("not: [valid").is_empty());
        assert!(collect_law_references("$id: wet_a\narticles: []\n").is_empty());
    }
}
