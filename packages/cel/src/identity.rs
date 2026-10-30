//! Who a cell is and what it answers: `<cell_dir>/cel.yaml`, with its
//! `name`, the `category` of authorities it belongs to (RFC-046), and its
//! catalogue of `answers` (RFC-048), and the reductions it keeps:
//! `<cell_dir>/reductions/<$id>/<valid_from>.yaml`.
//!
//! The law names who is competent for an article (`competent_authority`,
//! RFC-002): an organisation by its name (`INSTANCE`) or a kind of
//! organisation (`CATEGORY`). A cell's identity cannot come from the law, so
//! it is configuration the cell keeps. The engine compares the two as text.
//!
//! The catalogue names, per answer the cell gives, the reduction whose
//! output of that name it is: one of the cell's own, in its `reductions`
//! directory. It holds no logic: the reduction holds that, and the policy of
//! the authority that decides binds an open term to the answer
//! (`source.reduction`). Where a reduction sits says whose it is; a
//! reduction names no holder of its own (schema v0.8.12).
//!
//! ```yaml
//! name: Het college van burgemeester en wethouders van de gemeente Wilgendaal
//! category: college van burgemeester en wethouders
//! answers:
//!   aanvrager_heeft_geldige_gehandicaptenparkeerkaart: reductie_geldige_gehandicaptenparkeerkaart
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regelrecht_engine::Authority;
use serde::Deserialize;

use crate::error::{setup, Result};
use crate::files::Files;

/// The file in a cell directory that holds its identity.
pub const FILE: &str = "cel.yaml";

/// The directory in a cell directory that holds its reductions, one
/// directory per `$id` with a file per version (`<valid_from>.yaml`). Beside
/// the cells, a directory of this name holds the reductions of no cell, such
/// as that of the Awb, which every cell applies to its own chronicle: it is
/// no cell.
pub const REDUCTIONS: &str = "reductions";

/// Whether `dir` is the directory of the reductions of no cell, beside the
/// cells ([`REDUCTIONS`]), rather than the directory of a cell.
pub fn is_shared_reductions(dir: &Path) -> bool {
    dir.file_name().is_some_and(|name| name == REDUCTIONS)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    answers: BTreeMap<String, String>,
}

/// What the cell directory says: who the cell is, per answer it gives the
/// `$id` of the reduction whose output it is (`cel.yaml`), and the `$id`s
/// of the reductions it keeps (`reductions`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub authority: Authority,
    pub answers: BTreeMap<String, String>,
    pub reductions: BTreeSet<String>,
}

/// The identity of the cell in `dir`, read through `files`. A cell without
/// `cel.yaml`, with an unknown key, with an empty `name` or `category`, or
/// with an answer or reduction that is empty, is an error. So is an answer
/// whose reduction the cell does not keep, and a `reductions` directory
/// that holds anything but a directory per reduction with its versions.
pub fn read(files: &dyn Files, dir: &Path) -> Result<Identity> {
    let path = dir.join(FILE);
    let text = files.read(&path).map_err(|e| {
        setup(format!(
            "{}: a cell names who it is in {FILE} (name, category, answers): {e}",
            path.display()
        ))
    })?;
    let file: File =
        serde_yaml_ng::from_str(&text).map_err(|e| setup(format!("{}: {e}", path.display())))?;
    let blank = |s: &str| s.trim().is_empty();
    if blank(&file.name) || file.category.as_deref().is_some_and(blank) {
        return Err(setup(format!(
            "{}: a cell's name, and its category where given, may not be empty",
            path.display()
        )));
    }
    if let Some((answer, reduction)) = file
        .answers
        .iter()
        .find(|(answer, reduction)| blank(answer) || blank(reduction))
    {
        return Err(setup(format!(
            "{}: the answer '{answer}' names the reduction '{reduction}'; neither may be empty",
            path.display()
        )));
    }
    let reductions = reductions(files, &dir.join(REDUCTIONS))?;
    if let Some((answer, reduction)) = file
        .answers
        .iter()
        .find(|(_, reduction)| !reductions.contains(*reduction))
    {
        return Err(setup(format!(
            "{}: the answer '{answer}' names the reduction '{reduction}', which is not in \
             {}: a cell gives only the answers of its own reductions (RFC-048)",
            path.display(),
            dir.join(REDUCTIONS).display()
        )));
    }
    Ok(Identity {
        authority: Authority {
            name: file.name,
            category: file.category,
        },
        answers: file.answers,
        reductions,
    })
}

/// The `$id`s of the reductions in `dir`: the name of each directory in
/// it, which holds the versions of that reduction (`<valid_from>.yaml`).
/// No `dir` is no reductions. An entry that is no directory, and a
/// directory without a version or with anything else, is an error. The
/// same holds for the reductions of a cell and for those of no cell
/// beside the cells.
pub fn reductions(files: &dyn Files, dir: &Path) -> Result<BTreeSet<String>> {
    if !files.exists(dir) {
        return Ok(BTreeSet::new());
    }
    let mut ids = BTreeSet::new();
    for entry in files
        .entries(dir)
        .map_err(|e| setup(format!("{}: {e}", dir.display())))?
    {
        let versions = if files.is_dir(&entry) {
            files
                .entries(&entry)
                .map_err(|e| setup(format!("{}: {e}", entry.display())))?
        } else {
            Vec::new()
        };
        let is_version = |path: &std::path::PathBuf| {
            files.is_file(path)
                && path.extension().is_some_and(|x| x == "yaml")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
        };
        let id = entry.file_name().and_then(|n| n.to_str());
        match id {
            Some(id) if !versions.is_empty() && versions.iter().all(is_version) => {
                ids.insert(id.to_string());
            }
            _ => {
                return Err(setup(format!(
                    "{}: a directory of reductions holds one directory per reduction, named \
                     by its $id, with a file per version (<valid_from>.yaml) and nothing else",
                    entry.display()
                )))
            }
        }
    }
    Ok(ids)
}
