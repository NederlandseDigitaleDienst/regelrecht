//! A source made of directories elsewhere in a repository, by a manifest
//! (`bron.yaml`): read from a local checkout ([`lokaal`]) or from a GitHub
//! tarball ([`uit_tarball`], feature `github`). The poc-portal serves such a
//! source to a static poc, and the RegelHet component loads its one case
//! from one, so both read a manifest with this code.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// A source made of directories elsewhere in the repository: under
/// `directories`, per directory of the source, the directory of the
/// repository it is. `repo_root` is the path from the manifest's own
/// directory to the repository root (`../..` for a manifest two levels
/// down), so the manifest reads the same on a branch and in a local
/// checkout. A file lies under the longest directory of the
/// repository that holds it, so a subdirectory may land elsewhere than its
/// parent, such as a cell's `reducties` under `reductions`.
///
/// `ui_spec` names one file of the repository, under `repo_root`, that the
/// source holds at its root as [`UI_SPEC`]: the UI spec of the source,
/// from which the poc reads which field of an application is prefilled from
/// which source (poc-chronolexografie, `src/ui_spec.rs`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub repo_root: String,
    pub directories: BTreeMap<String, String>,
    #[serde(default)]
    pub ui_spec: Option<String>,
}

/// Where the source holds the file `ui_spec` of a manifest names.
pub const UI_SPEC: &str = "ui_spec.yaml";

impl Manifest {
    /// Whether `path` names a manifest rather than a folder.
    pub fn is_manifest(path: &str) -> bool {
        path.ends_with(".yaml") || path.ends_with(".yml")
    }

    /// The manifest in `tekst`, read from `waar` (for the message). Every
    /// directory it names lies below the root (no `..`, `.` or empty part),
    /// and no directory of the repository is named twice.
    ///
    /// # Errors
    ///
    /// When `tekst` is no manifest, or names a directory that is not below
    /// the root, a UI spec that is no YAML file or lies in a directory of the
    /// source, or one directory twice.
    pub fn lees(tekst: &str, waar: &str) -> Result<Self, String> {
        let m: Self =
            serde_yaml_ng::from_str(tekst).map_err(|e| format!("the manifest {waar}: {e}"))?;
        for pad in m
            .directories
            .keys()
            .chain(m.directories.values())
            .chain(&m.ui_spec)
        {
            let pad = pad.trim_matches('/');
            if pad.is_empty()
                || pad
                    .split('/')
                    .any(|d| d == ".." || d == "." || d.is_empty())
            {
                return Err(format!(
                    "the manifest {waar}: '{pad}' is no directory below the root"
                ));
            }
        }
        // The UI spec is one YAML file, as the portal reads only YAML from
        // a branch, and lies outside every directory the source is made of:
        // there it would also be read as a case or a regelwerk.
        if let Some(pad) = &m.ui_spec {
            let pad = pad.trim_matches('/');
            if !Self::is_manifest(pad) {
                return Err(format!(
                    "the manifest {waar}: the UI spec '{pad}' is no YAML file (.yaml or .yml)"
                ));
            }
            if let Some(van) = m
                .directories
                .values()
                .map(|v| v.trim_matches('/'))
                .find(|van| pad.starts_with(&format!("{van}/")))
            {
                return Err(format!(
                    "the manifest {waar}: the UI spec '{pad}' lies in '{van}', a directory of the source"
                ));
            }
        }
        // One directory of the repository lands in one place: two that name
        // the same would leave the choice to the order of reading.
        let mut gezien = std::collections::BTreeSet::new();
        for van in m.directories.values() {
            if !gezien.insert(van.trim_matches('/')) {
                return Err(format!(
                    "the manifest {waar}: '{}' is named twice",
                    van.trim_matches('/')
                ));
            }
        }
        Ok(m)
    }

    /// The repository root, as a path under the repository, of the manifest
    /// at `pad` (a path under the repository); `None` when `repo_root`
    /// climbs out.
    #[cfg(feature = "github")]
    fn repo_root_of(&self, pad: &str) -> Option<String> {
        let mut delen: Vec<&str> = pad.split('/').collect();
        delen.pop();
        for d in self
            .repo_root
            .split('/')
            .filter(|d| !d.is_empty() && *d != ".")
        {
            if d == ".." {
                delen.pop()?;
            } else {
                delen.push(d);
            }
        }
        Some(delen.join("/"))
    }

    /// The source from `alle`, the files of the repository by their path
    /// under the root: each file under a directory the manifest names, by
    /// its path in the source. A directory without a YAML file is an error
    /// that names it.
    ///
    /// # Errors
    ///
    /// When a directory the manifest names holds no YAML file, or the UI spec
    /// it names is not in `alle`.
    pub fn bron(
        &self,
        alle: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut uit = BTreeMap::new();
        for (pad, tekst) in alle {
            let langste = self
                .directories
                .iter()
                .map(|(doel, van)| (doel.trim_matches('/'), van.trim_matches('/')))
                .filter(|(_, van)| pad.starts_with(&format!("{van}/")))
                .max_by_key(|(_, van)| van.len());
            if let Some((doel, van)) = langste {
                uit.insert(format!("{doel}{}", &pad[van.len()..]), tekst.clone());
            }
        }
        for (doel, van) in &self.directories {
            let van = van.trim_matches('/');
            if !alle.keys().any(|p| p.starts_with(&format!("{van}/"))) {
                return Err(format!(
                    "the manifest names '{van}' for '{doel}', which holds no YAML file"
                ));
            }
        }
        if let Some(pad) = &self.ui_spec {
            let pad = pad.trim_matches('/');
            let tekst = alle.get(pad).ok_or_else(|| {
                format!(
                    "the manifest names '{pad}' for its UI spec, which the branch does not hold"
                )
            })?;
            uit.insert(UI_SPEC.to_string(), tekst.clone());
        }
        Ok(uit)
    }
}

/// The YAML files under `path` in a GitHub tarball, by their path under it,
/// or the source its manifest makes when `path` names one ([`Manifest`]),
/// and the short sha of the commit (seven characters from the end of the
/// archive's top-level directory, `{owner}-{repo}-{sha}`). No file under `path` is an error: the
/// folder is not on the branch.
///
/// # Errors
///
/// When the tarball does not read, holds no YAML file under `path`, or the
/// manifest it names does not read or make a source ([`Manifest::bron`]).
#[cfg(feature = "github")]
pub fn uit_tarball(
    bytes: &[u8],
    path: &str,
) -> Result<(Option<String>, BTreeMap<String, String>), String> {
    let path = path.trim_matches('/');
    let manifest = Manifest::is_manifest(path);
    let prefix = format!("{path}/");
    let mut bestanden = BTreeMap::new();
    let top = crate::github::each_yaml_in_tar_gz(bytes, |rel, text| {
        if manifest {
            bestanden.insert(rel.to_string(), text);
        } else if let Some(onder) = rel.strip_prefix(&prefix) {
            bestanden.insert(onder.to_string(), text);
        }
    })
    .map_err(|e| format!("the tarball does not read: {e}"))?;
    if manifest {
        let tekst = bestanden
            .get(path)
            .ok_or_else(|| format!("the branch has no manifest '{path}'"))?;
        let m = Manifest::lees(tekst, path)?;
        let repo_root = m.repo_root_of(path).ok_or_else(|| {
            format!("the manifest {path}: its repo_root lies above the repository")
        })?;
        let onder = if repo_root.is_empty() {
            bestanden
        } else {
            let prefix = format!("{repo_root}/");
            bestanden
                .into_iter()
                .filter_map(|(p, t)| p.strip_prefix(&prefix).map(|o| (o.to_string(), t)))
                .collect()
        };
        bestanden = m.bron(&onder)?;
    }
    if bestanden.is_empty() {
        return Err(format!("the branch has no YAML file under '{path}'"));
    }
    let versie = top.and_then(|t| {
        t.rsplit('-')
            .next()
            .map(|sha| sha.chars().take(7).collect())
    });
    Ok((versie, bestanden))
}

/// The source at `pad` on disk: the YAML files of a folder ([`uit_map`]),
/// or what a manifest makes of the directories it names ([`Manifest`]).
///
/// # Errors
///
/// When the folder or the manifest does not read, or the manifest makes no
/// source ([`Manifest::bron`]).
pub fn lokaal(pad: &Path) -> Result<BTreeMap<String, String>, String> {
    if !Manifest::is_manifest(&pad.to_string_lossy()) {
        return uit_map(pad);
    }
    let tekst = std::fs::read_to_string(pad).map_err(|e| format!("{}: {e}", pad.display()))?;
    let m = Manifest::lees(&tekst, &pad.display().to_string())?;
    let repo_root = pad.parent().unwrap_or(Path::new("")).join(&m.repo_root);
    let mut alle = BTreeMap::new();
    for van in m.directories.values() {
        let van = van.trim_matches('/');
        let dir = repo_root.join(van);
        if !dir.is_dir() {
            continue;
        }
        for (onder, tekst) in uit_map(&dir)? {
            alle.insert(format!("{van}/{onder}"), tekst);
        }
    }
    if let Some(pad) = &m.ui_spec {
        let pad = pad.trim_matches('/');
        let bestand = repo_root.join(pad);
        match std::fs::read_to_string(&bestand) {
            Ok(tekst) => {
                alle.insert(pad.to_string(), tekst);
            }
            // Not there: `bron` names it, as on a branch.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", bestand.display())),
        }
    }
    m.bron(&alle)
}

/// The YAML files under the folder `dir`, by their path under it.
///
/// # Errors
///
/// When `dir` or a file under it does not read.
pub fn uit_map(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    fn loop_door(
        dir: &Path,
        rel: &Path,
        uit: &mut BTreeMap<String, String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir.join(rel))? {
            let entry = entry?;
            let rel = rel.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                loop_door(dir, &rel, uit)?;
            } else if rel.extension().is_some_and(|x| x == "yaml" || x == "yml") {
                let text = std::fs::read_to_string(dir.join(&rel))?;
                uit.insert(rel.to_string_lossy().replace('\\', "/"), text);
            }
        }
        Ok(())
    }
    let mut uit = BTreeMap::new();
    loop_door(dir, Path::new(""), &mut uit).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(uit)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// A gzipped tar laid out as GitHub's tarball endpoint lays it out.
    #[cfg(feature = "github")]
    fn tarball(top: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        // GitHub's archive opens with a pax global header, before the
        // top-level directory.
        let comment = b"52 comment=0123456789abcdef0123456789abcdef01234567\n";
        let mut pax = tar::Header::new_ustar();
        pax.set_entry_type(tar::EntryType::XGlobalHeader);
        pax.set_size(comment.len() as u64);
        pax.set_mode(0o644);
        tar.append_data(&mut pax, "pax_global_header", &comment[..])
            .unwrap();
        let mut dir = tar::Header::new_gnu();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        tar.append_data(&mut dir, format!("{top}/"), std::io::empty())
            .unwrap();
        for (path, text) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(text.len() as u64);
            header.set_mode(0o644);
            tar.append_data(&mut header, format!("{top}/{path}"), text.as_bytes())
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    #[cfg(feature = "github")]
    fn a_tarball_becomes_the_yaml_files_under_the_folder() {
        let bytes = tarball(
            "example-org-regelrecht-corpus-example-abc1234",
            &[
                ("x/casussen/c.yaml", "cel: c"),
                ("x/regulation/nl/w/2026-01-01.yaml", "$id: w"),
                ("x/README.md", "not yaml"),
                ("y/elders.yaml", "not under x"),
                ("x-other/z.yaml", "a sibling, not under x"),
            ],
        );
        let (versie, bestanden) = uit_tarball(&bytes, "x").unwrap();
        assert_eq!(versie.as_deref(), Some("abc1234"));
        assert_eq!(
            bestanden,
            BTreeMap::from([
                ("casussen/c.yaml".to_string(), "cel: c".to_string()),
                (
                    "regulation/nl/w/2026-01-01.yaml".to_string(),
                    "$id: w".to_string()
                ),
            ])
        );
    }

    #[test]
    #[cfg(feature = "github")]
    fn a_tarball_without_the_folder_is_an_error() {
        let bytes = tarball("o-r-abc", &[("y/a.yaml", "a: 1")]);
        let e = uit_tarball(&bytes, "x").unwrap_err();
        assert!(e.contains("'x'"), "{e}");
        assert!(uit_tarball(b"no tarball", "x").is_err());
    }

    #[test]
    fn a_local_folder_becomes_its_yaml_files() {
        let dir = std::env::temp_dir().join(format!("poc-bron-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("casussen")).unwrap();
        std::fs::write(dir.join("casussen/c.yaml"), "cel: c").unwrap();
        std::fs::write(dir.join("notes.txt"), "no").unwrap();
        let bestanden = uit_map(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            bestanden,
            BTreeMap::from([("casussen/c.yaml".to_string(), "cel: c".to_string())])
        );
        assert!(uit_map(Path::new("/nonexistent/poc-bron")).is_err());
    }

    #[cfg(feature = "github")]
    const MANIFEST: &str = "repo_root: ../..\ndirectories:\n  regulation/nl/w: regulation/nl/w\n  regulation/nl/v: werk/eigen/regulation/nl/v\n  cells/c: werk/m/cellen/c\n  cells/c/reductions: werk/m/cellen/c/reducties\n  casussen: werk/m/casussen\n";

    #[test]
    #[cfg(feature = "github")]
    fn a_manifest_in_a_tarball_makes_the_source_from_its_directories() {
        let bytes = tarball(
            "o-r-abc1234",
            &[
                ("werk/m/bron.yaml", MANIFEST),
                ("regulation/nl/w/2026-01-01.yaml", "$id: w"),
                ("regulation/nl/v/2026-01-01.yaml", "$id: v van Luc"),
                ("werk/eigen/regulation/nl/v/2026-01-01.yaml", "$id: v"),
                ("werk/m/cellen/c/cel.yaml", "name: C"),
                ("werk/m/cellen/c/reducties/r/2026-01-01.yaml", "$id: r"),
                ("werk/m/casussen/k.yaml", "cel: c"),
                ("werk/groot/niet.yaml", "not named"),
            ],
        );
        let (versie, bestanden) = uit_tarball(&bytes, "werk/m/bron.yaml").unwrap();
        assert_eq!(versie.as_deref(), Some("abc1234"));
        let paden: Vec<&str> = bestanden.keys().map(String::as_str).collect();
        assert_eq!(
            paden,
            [
                "casussen/k.yaml",
                "cells/c/cel.yaml",
                "cells/c/reductions/r/2026-01-01.yaml",
                "regulation/nl/v/2026-01-01.yaml",
                "regulation/nl/w/2026-01-01.yaml",
            ]
        );
        assert_eq!(bestanden["regulation/nl/v/2026-01-01.yaml"], "$id: v");
    }

    #[test]
    #[cfg(feature = "github")]
    fn a_manifest_names_what_it_cannot_find() {
        let bytes = tarball("o-r-abc", &[("werk/m/bron.yaml", MANIFEST)]);
        let e = uit_tarball(&bytes, "werk/m/bron.yaml").unwrap_err();
        assert!(e.contains("holds no YAML file"), "{e}");
        let e = uit_tarball(&bytes, "werk/m/ander.yaml").unwrap_err();
        assert!(e.contains("no manifest 'werk/m/ander.yaml'"), "{e}");
        let boven = tarball(
            "o-r-abc",
            &[("m.yaml", "repo_root: ..\ndirectories: {a: b}\n")],
        );
        let e = uit_tarball(&boven, "m.yaml").unwrap_err();
        assert!(e.contains("above the repository"), "{e}");
        assert!(Manifest::lees("directories: {a: ../b}", "m").is_err());
        let e = Manifest::lees("directories: {a: b, c: b/}", "m").unwrap_err();
        assert!(e.contains("named twice"), "{e}");
    }

    #[test]
    #[cfg(feature = "github")]
    fn a_manifest_places_its_ui_spec_at_the_root_of_the_source() {
        let manifest = format!("{MANIFEST}ui_spec: werk/ui-spec.yaml\n");
        let files = [
            ("werk/m/bron.yaml", manifest.as_str()),
            ("regulation/nl/w/2026-01-01.yaml", "$id: w"),
            ("werk/eigen/regulation/nl/v/2026-01-01.yaml", "$id: v"),
            ("werk/m/cellen/c/cel.yaml", "name: C"),
            ("werk/m/cellen/c/reducties/r/2026-01-01.yaml", "$id: r"),
            ("werk/m/casussen/k.yaml", "cel: c"),
            ("werk/ui-spec.yaml", "elementen: []"),
        ];
        let (_, bestanden) = uit_tarball(&tarball("o-r-abc", &files), "werk/m/bron.yaml").unwrap();
        assert_eq!(bestanden[UI_SPEC], "elementen: []");
        // Read locally, the same.
        let dir = std::env::temp_dir().join(format!("poc-bron-ui-{}", std::process::id()));
        for (pad, tekst) in files {
            let pad = dir.join(pad);
            std::fs::create_dir_all(pad.parent().unwrap()).unwrap();
            std::fs::write(pad, tekst).unwrap();
        }
        let lokaal = lokaal(&dir.join("werk/m/bron.yaml"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(lokaal.unwrap(), bestanden);
        // A UI spec the branch does not hold is named.
        let zonder = &files[..files.len() - 1];
        let e = uit_tarball(&tarball("o-r-abc", zonder), "werk/m/bron.yaml").unwrap_err();
        assert!(e.contains("'werk/ui-spec.yaml' for its UI spec"), "{e}");
        // It lies below the root, as every directory does; it is a YAML file;
        // and it lies outside the directories of the source.
        assert!(Manifest::lees("directories: {a: b}\nui_spec: ../u.yaml", "m").is_err());
        let e = Manifest::lees("directories: {a: b}\nui_spec: u.json", "m").unwrap_err();
        assert!(e.contains("no YAML file"), "{e}");
        let e = Manifest::lees("directories: {a: b}\nui_spec: b/u.yaml", "m").unwrap_err();
        assert!(e.contains("lies in 'b'"), "{e}");
    }

    #[test]
    fn a_manifest_with_the_old_dutch_field_names_is_refused() {
        let e = Manifest::lees("wortel: ../..\ndirectories: {a: b}\n", "m").unwrap_err();
        assert!(e.contains("unknown field `wortel`"), "{e}");
        let e = Manifest::lees("repo_root: ../..\nmappen: {a: b}\n", "m").unwrap_err();
        assert!(e.contains("unknown field `mappen`"), "{e}");
    }

    #[test]
    #[cfg(feature = "github")]
    fn a_local_manifest_reads_as_the_one_on_the_branch() {
        let dir = std::env::temp_dir().join(format!("poc-bron-manifest-{}", std::process::id()));
        let schrijf = |pad: &str, tekst: &str| {
            let pad = dir.join(pad);
            std::fs::create_dir_all(pad.parent().unwrap()).unwrap();
            std::fs::write(pad, tekst).unwrap();
        };
        let files = [
            ("werk/m/bron.yaml", MANIFEST),
            ("regulation/nl/w/2026-01-01.yaml", "$id: w"),
            ("werk/eigen/regulation/nl/v/2026-01-01.yaml", "$id: v"),
            ("werk/m/cellen/c/cel.yaml", "name: C"),
            ("werk/m/cellen/c/reducties/r/2026-01-01.yaml", "$id: r"),
            ("werk/m/casussen/k.yaml", "cel: c"),
        ];
        for (pad, tekst) in files {
            schrijf(pad, tekst);
        }
        let lokaal = lokaal(&dir.join("werk/m/bron.yaml"));
        let (_, uit_branch) = uit_tarball(&tarball("o-r-abc", &files), "werk/m/bron.yaml").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(lokaal.unwrap(), uit_branch);
    }
}
