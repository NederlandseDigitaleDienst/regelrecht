//! The bundle the app plays from in the browser: the files every case needs,
//! by their path in the repository, built into the page.
//!
//! Which directories those are follows from the case files and the law
//! ([`mappen`]): the case files, per case the regelwerken its cell runs, and
//! its cells. The register (`pocs/registry.yaml`, `corpus` of
//! `chronolexografie`) lists them for the build, which copies every file
//! under them except the scenarios; a test below holds that list equal to
//! [`mappen`], so neither can drift.
//!
//! Next to it the page may hold sources ([`Bron`]): a corpus the portal
//! fetches when the page loads, such as a branch of another repository
//! (`sources` of `chronolexografie` in the register). A source holds its own
//! cases (`casussen/`), regelwerken (`regulation/nl/`) and cells (`cells/`,
//! with their reductions).
//! A case from a source runs on that source and the regelrecht corpus; the
//! cases of this crate never see a source ([`met_bronnen`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_cel::{identity, Bundle, Error, Files};
use serde::{Deserialize, Serialize};

use crate::casus::{casussen as lees_casussen, Regelwerken};
use crate::kern::{Corpus, Result};

/// The corpus, under the repository root: the regelwerken under
/// `regulation/nl` and the cells under `cells`, with their reductions.
pub const CORPUS: &str = "corpus";

/// The case files, under the repository root.
pub const CASUSSEN: &str = "packages/poc-chronolexografie/casussen";

/// Every case on the bundle `files`, each a path under the repository root
/// with its text.
pub fn casussen(files: BTreeMap<String, String>) -> Result<Vec<Corpus>> {
    let bundle = Bundle::new(files).map_err(|e| Error::Setup(format!("the bundle: {e}")))?;
    Corpus::alle_in(
        Arc::new(bundle),
        &[PathBuf::from(CORPUS)],
        Path::new(CASUSSEN),
    )
}

/// Where a source's files are in the bundle a case from it plays on:
/// `bron/<name>/...`, next to the repository paths of the regelrecht files.
pub const BRONNEN: &str = "bron";

/// A source as the page gives it: its files by their path in the source, or
/// why the page could not get them (`fout`), such as a portal without the
/// token. `versie` says which state of the source it is, for the visitor;
/// `melding` what the portal says about it, such as that this is its last
/// good copy because GitHub failed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bron {
    pub naam: String,
    #[serde(default)]
    pub versie: Option<String>,
    #[serde(default)]
    pub bestanden: BTreeMap<String, String>,
    #[serde(default)]
    pub fout: Option<String>,
    #[serde(default)]
    pub melding: Option<String>,
}

/// What became of a source, as the app shows it: unavailable or broken
/// (`fout`, with the file it names), a case from it that did not load
/// (`fouten`), and what loading set aside (`meldingen`), such as a `$id`
/// the regelrecht corpus holds too.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct BronStand {
    pub naam: String,
    pub versie: Option<String>,
    pub fout: Option<String>,
    pub fouten: Vec<String>,
    pub meldingen: Vec<String>,
    /// The ids of the cases it adds.
    pub casussen: Vec<String>,
}

/// Every case on the bundle `files` ([`casussen`]), and every case of the
/// `bronnen`, each on the regelrecht files and its own source. A broken
/// regelrecht bundle fails as before; a broken source does not: what went
/// wrong is in its [`BronStand`], and the other cases play on. A case from
/// a source with the id of a case before it is set aside, with a message.
pub fn met_bronnen(
    files: BTreeMap<String, String>,
    bronnen: Vec<Bron>,
) -> Result<(Vec<Corpus>, Vec<BronStand>)> {
    let mut alle = casussen(files.clone())?;
    let mut standen = Vec::new();
    for bron in bronnen {
        let mut stand = BronStand {
            naam: bron.naam.clone(),
            versie: bron.versie.clone(),
            fout: bron.fout.clone(),
            meldingen: bron.melding.iter().cloned().collect(),
            ..BronStand::default()
        };
        if stand.fout.is_none() {
            match uit_bron(&files, &bron, &mut stand) {
                Ok(erbij) => {
                    for corpus in erbij {
                        let id = corpus.casus.id.clone();
                        if alle.iter().any(|c| c.casus.id == id) {
                            stand.meldingen.push(format!(
                                "case '{id}' is also a case before this source; the app plays that one"
                            ));
                            continue;
                        }
                        stand.casussen.push(id);
                        alle.push(corpus);
                    }
                }
                Err(e) => stand.fout = Some(e.to_string()),
            }
        }
        standen.push(stand);
    }
    Ok((alle, standen))
}

/// The cases of `bron`, each on the regelrecht `files` and the source under
/// `bron/<name>`. A case that does not load is in `stand.fouten`.
fn uit_bron(
    files: &BTreeMap<String, String>,
    bron: &Bron,
    stand: &mut BronStand,
) -> Result<Vec<Corpus>> {
    let wortel = Path::new(BRONNEN).join(&bron.naam);
    // A name is one path component of a-z, 0-9 and '-', so it stays under
    // `bron/`.
    let geldig = bron
        .naam
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if bron.naam.is_empty() || !geldig {
        return Err(Error::Setup(format!(
            "'{}' is no name of a source",
            bron.naam
        )));
    }
    let mut alles = files.clone();
    for (pad, tekst) in &bron.bestanden {
        let pad = format!("{}/{pad}", wortel.display());
        if alles.insert(pad.clone(), tekst.clone()).is_some() {
            return Err(Error::Setup(format!("'{pad}' is twice in the bundle")));
        }
    }
    let bundle: Arc<Bundle> = Arc::new(
        Bundle::new(alles).map_err(|e| Error::Setup(format!("source '{}': {e}", bron.naam)))?,
    );
    let roots = [PathBuf::from(CORPUS), wortel.clone()];
    let regelwerken = Regelwerken::load(&*bundle, &roots)?;
    stand
        .meldingen
        .extend(regelwerken.meldingen.iter().cloned());
    // A cell of the source with the name of a regelrecht cell: the case
    // runs the regelrecht one (the first root wins), and says so.
    let cellen = wortel.join("cells");
    if bundle.is_dir(&cellen) {
        for dir in bundle.entries(&cellen)? {
            let Some(naam) = dir.file_name() else {
                continue;
            };
            // The reductions of no cell are regelwerken, which loading
            // compares by $id.
            if identity::is_shared_reductions(&dir) {
                continue;
            }
            let ook = Path::new(CORPUS).join("cells").join(naam);
            if bundle.is_dir(&ook) {
                stand.meldingen.push(format!(
                    "cell '{}' in {} is also in {}; the app runs the latter",
                    naam.to_string_lossy(),
                    dir.display(),
                    ook.display()
                ));
            }
        }
    }
    let mut uit = Vec::new();
    for mut casus in lees_casussen(&*bundle, &wortel.join("casussen"))? {
        casus.bron = Some(bron.naam.clone());
        let id = casus.id.clone();
        match Corpus::load(bundle.clone(), &roots, &regelwerken, casus) {
            Ok(c) => uit.push(c),
            Err(e) => stand.fouten.push(format!("case '{id}': {e}")),
        }
    }
    Ok(uit)
}

/// The directories, under the repository root, that the bundle of `alle`
/// holds: the case files, and per case the directories of the regelwerken
/// its cell runs and of its own cell and the other cells of its sphere
/// (RFC-052).
pub fn mappen(alle: &[Corpus]) -> BTreeSet<String> {
    let mut uit = BTreeSet::from([CASUSSEN.to_string()]);
    for c in alle {
        uit.extend(c.mappen().map(|m| format!("{CORPUS}/{m}")));
        let cellen =
            std::iter::once(&c.casus.cel).chain(c.casus.andere_cellen.iter().map(|a| &a.cel));
        uit.extend(cellen.map(|cel| format!("{CORPUS}/cells/{cel}")));
    }
    // The reductions of a cell are in its directory already.
    let binnen = |m: &String| {
        uit.iter()
            .any(|ander| ander != m && m.starts_with(&format!("{ander}/")))
    };
    uit.iter().filter(|m| !binnen(m)).cloned().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The `corpus` the register lists for this app.
    fn in_het_register() -> BTreeSet<String> {
        #[derive(serde::Deserialize)]
        struct Register {
            pocs: Vec<Poc>,
        }
        #[derive(serde::Deserialize)]
        struct Poc {
            slug: String,
            #[serde(default)]
            corpus: Vec<String>,
        }
        let yaml = std::fs::read_to_string(repo().join("pocs/registry.yaml")).unwrap();
        let register: Register = serde_yaml_ng::from_str(&yaml).unwrap();
        let poc = register
            .pocs
            .into_iter()
            .find(|p| p.slug == "chronolexografie")
            .expect("chronolexografie in pocs/registry.yaml");
        poc.corpus.into_iter().collect()
    }

    /// Every file under `dir` except the scenarios, as the build bundles
    /// them (frontend/scripts/copy-assets.js).
    fn verzamel(dir: &Path, uit: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(repo().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            let rel = dir.join(path.file_name().unwrap());
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "scenarios") {
                    verzamel(&rel, uit);
                }
            } else {
                let text = std::fs::read_to_string(&path).unwrap();
                uit.insert(rel.to_string_lossy().replace('\\', "/"), text);
            }
        }
    }

    #[test]
    fn the_register_lists_what_every_case_needs_and_nothing_else() {
        let alle = Corpus::alle(&repo().join(CORPUS), &repo().join(CASUSSEN)).unwrap();
        assert_eq!(in_het_register(), mappen(&alle));
    }

    #[test]
    fn the_bundle_plays_every_case_as_the_whole_corpus_does() {
        // The derivation over the bundle sees only the directories the
        // register lists, and still finds what it finds over the whole
        // corpus: nothing a case needs is missing from the bundle.
        let mut files = BTreeMap::new();
        for dir in in_het_register() {
            verzamel(Path::new(&dir), &mut files);
        }
        let gebundeld = casussen(files).unwrap();
        let op_schijf = Corpus::alle(&repo().join(CORPUS), &repo().join(CASUSSEN)).unwrap();
        assert_eq!(gebundeld.len(), op_schijf.len());
        for (b, d) in gebundeld.iter().zip(&op_schijf) {
            assert_eq!(b.beschrijving(), d.beschrijving(), "{}", d.casus.id);
            let lengtes = |c: &Corpus| -> Vec<usize> {
                c.cell().unwrap().kronieken().iter().map(Vec::len).collect()
            };
            assert_eq!(lengtes(b), lengtes(d), "{}", d.casus.id);
        }
    }

    #[test]
    fn the_deploy_filter_holds_what_every_case_runs() {
        // A change to a regelwerk or a cell a case runs rebuilds the image
        // that bundles it. The case files are in this crate's directory,
        // which the filter takes from the cargo graph.
        let filters = std::fs::read_to_string(repo().join("script/deploy-filters.mjs")).unwrap();
        for pad in in_het_register()
            .into_iter()
            .filter(|p| p.as_str() != CASUSSEN)
        {
            assert!(
                filters.contains(&format!("'{pad}/'")),
                "{pad}/ not in deploy-filters.mjs"
            );
        }
    }

    #[test]
    fn a_bundle_without_a_file_a_case_needs_fails_at_the_start() {
        let mut files = BTreeMap::new();
        for dir in in_het_register() {
            verzamel(Path::new(&dir), &mut files);
        }
        // Without its catalogue the school has no cell.
        let mut without_cell = files.clone();
        without_cell.remove("corpus/cells/obs_de_klimop/cel.yaml");
        let e = casussen(without_cell).unwrap_err().to_string();
        assert!(e.contains("corpus/cells/obs_de_klimop"), "{e}");
        // Without a reduction its catalogue names, the cell does not open.
        files.remove(
            "corpus/cells/obs_de_klimop/reductions/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml",
        );
        let e = casussen(files).unwrap_err().to_string();
        assert!(
            e.contains("reductie_verlof_al_verleend_in_schooljaar")
                && e.contains("only the answers of its own reductions"),
            "{e}"
        );
    }

    /// The regelrecht bundle as the build makes it.
    fn gebundeld() -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        for dir in in_het_register() {
            verzamel(Path::new(&dir), &mut files);
        }
        files
    }

    /// Every YAML file under `dir`, by its path under `dir`, as the portal
    /// gives a source (packages/poc-portal/src/bron.rs).
    fn uit_map(dir: &Path) -> BTreeMap<String, String> {
        fn loop_door(dir: &Path, rel: &Path, uit: &mut BTreeMap<String, String>) {
            for entry in std::fs::read_dir(dir.join(rel)).unwrap() {
                let path = entry.unwrap().path();
                let rel = rel.join(path.file_name().unwrap());
                if path.is_dir() {
                    loop_door(dir, &rel, uit);
                } else if path.extension().is_some_and(|x| x == "yaml" || x == "yml") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    uit.insert(rel.to_string_lossy().replace('\\', "/"), text);
                }
            }
        }
        let mut uit = BTreeMap::new();
        loop_door(dir, Path::new(""), &mut uit);
        uit
    }

    /// The test source in `tests/bron-voorbeeld`.
    fn voorbeeld() -> Bron {
        Bron {
            naam: "voorbeeld".into(),
            versie: Some("abc1234".into()),
            bestanden: uit_map(&repo().join("packages/poc-chronolexografie/tests/bron-voorbeeld")),
            ..Bron::default()
        }
    }

    /// Play the case `id` as its example says: apply, and decide with the
    /// example findings; the decision's outputs.
    fn speel(spel: &mut crate::spel::Spel, id: &str) -> serde_json::Value {
        let casus = spel.kiezen(id).unwrap();
        let map = |v: &serde_json::Value| v.as_object().cloned().unwrap_or_default();
        let dag = casus["speeldatum"].as_str().unwrap().parse().unwrap();
        let gram = spel
            .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), dag)
            .unwrap();
        let besluit = spel
            .besluiten(
                gram["id"].as_str().unwrap(),
                &map(&casus["voorbeeld"]["bevindingen"]),
                dag,
            )
            .unwrap();
        besluit["besluit"]["outputs"].clone()
    }

    #[test]
    fn a_case_from_a_source_plays_to_a_decision() {
        let (alle, standen) = met_bronnen(gebundeld(), vec![voorbeeld()]).unwrap();
        let mut spel = crate::spel::Spel::met_bronnen(alle, standen);
        let lijst = spel.casussen().unwrap();
        assert_eq!(lijst["bronnen"][0]["naam"], "voorbeeld");
        let outputs = speel(&mut spel, "voorbeeld_subsidie");
        assert_eq!(outputs["subsidie_verleend"], serde_json::json!(true));
    }

    /// With `POC_BRON_NAPP` set to a local source folder (as for `just
    /// poc-chronolexografie`), every case in it loads and plays its example to
    /// a decision. Without it there is nothing to check.
    #[test]
    fn the_local_source_in_poc_bron_napp_plays() {
        let Some(dir) = std::env::var_os("POC_BRON_NAPP") else {
            return;
        };
        // A relative path counts from the repository root, as for `just`.
        let dir = repo().join(dir);
        let bron = Bron {
            naam: "napp".into(),
            versie: Some("lokaal".into()),
            bestanden: uit_map(&dir),
            ..Bron::default()
        };
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        let stand = &standen[0];
        assert!(stand.fout.is_none() && stand.fouten.is_empty(), "{stand:?}");
        assert!(!stand.casussen.is_empty(), "{stand:?}");
        let ids = stand.casussen.clone();
        let mut spel = crate::spel::Spel::met_bronnen(alle, standen);
        for id in ids {
            speel(&mut spel, &id);
        }
    }

    #[test]
    fn a_case_from_a_source_plays_on_the_source_and_the_regelrecht_corpus() {
        let (alle, standen) = met_bronnen(gebundeld(), vec![voorbeeld()]).unwrap();
        let c = alle
            .iter()
            .find(|c| c.casus.id == "voorbeeld_subsidie")
            .expect("the case of the source");
        assert_eq!(c.casus.bron.as_deref(), Some("voorbeeld"));
        // The regulation and the cell from the source, the objection from
        // the regelrecht Awb.
        assert_eq!(c.afleiding.regelwerken[0], "voorbeeld_bron_subsidie");
        assert_eq!(c.afleiding.objection_law, "algemene_wet_bestuursrecht");
        assert_eq!(c.afleiding.rol, "Voorbeeldautoriteit");
        assert!(c.cell().is_ok());
        assert_eq!(
            standen,
            vec![BronStand {
                naam: "voorbeeld".into(),
                versie: Some("abc1234".into()),
                casussen: vec!["voorbeeld_subsidie".into()],
                ..BronStand::default()
            }]
        );
    }

    #[test]
    fn the_cases_of_this_crate_do_not_see_a_source() {
        let zonder = casussen(gebundeld()).unwrap();
        let (met, _) = met_bronnen(gebundeld(), vec![voorbeeld()]).unwrap();
        assert_eq!(met.len(), zonder.len() + 1);
        for (m, z) in met.iter().zip(&zonder) {
            assert_eq!(m.casus.id, z.casus.id);
            assert_eq!(m.casus.bron, None);
            assert_eq!(m.afleiding.regelwerken, z.afleiding.regelwerken);
        }
    }

    #[test]
    fn an_unavailable_source_leaves_the_other_cases_alone() {
        let weg = Bron {
            naam: "napp".into(),
            fout: Some("bron niet beschikbaar".into()),
            ..Bron::default()
        };
        let (alle, standen) = met_bronnen(gebundeld(), vec![weg]).unwrap();
        assert_eq!(alle.len(), casussen(gebundeld()).unwrap().len());
        assert_eq!(standen[0].fout.as_deref(), Some("bron niet beschikbaar"));
        assert!(standen[0].casussen.is_empty());
    }

    #[test]
    fn a_broken_file_in_a_source_names_the_file() {
        let mut bron = voorbeeld();
        let pad = "regulation/nl/uitvoeringsbeleid/voorbeeld_bron_subsidie/2026-01-01.yaml";
        bron.bestanden.insert(pad.into(), "$id: [kapot".into());
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(alle.iter().all(|c| c.casus.bron.is_none()));
        let fout = standen[0].fout.clone().unwrap();
        assert!(fout.contains(&format!("bron/voorbeeld/{pad}")), "{fout}");
    }

    #[test]
    fn a_case_that_does_not_load_is_named_and_the_source_plays_on() {
        let mut bron = voorbeeld();
        bron.bestanden.insert(
            "casussen/kapot.yaml".into(),
            "cel: voorbeeld_autoriteit\n".into(),
        );
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        // A case file that does not parse fails the source; it names the file.
        assert!(alle.iter().all(|c| c.casus.bron.is_none()));
        let fout = standen[0].fout.clone().unwrap();
        assert!(
            fout.contains("bron/voorbeeld/casussen/kapot.yaml"),
            "{fout}"
        );

        let mut bron = voorbeeld();
        let casus = bron.bestanden["casussen/voorbeeld_subsidie.yaml"].replace(
            "artikel: voorbeeld_bron_subsidie#1",
            "artikel: voorbeeld_bron_subsidie#9",
        );
        bron.bestanden.insert("casussen/ander.yaml".into(), casus);
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert_eq!(standen[0].casussen, vec!["voorbeeld_subsidie".to_string()]);
        assert_eq!(standen[0].fouten.len(), 1, "{:?}", standen[0]);
        assert!(
            standen[0].fouten[0].contains("case 'ander'"),
            "{:?}",
            standen[0]
        );
    }

    #[test]
    fn the_regelrecht_corpus_wins_over_a_source_with_the_same_id() {
        let mut bron = voorbeeld();
        let awb = "corpus/regulation/nl/wet/algemene_wet_bestuursrecht/1994-01-01.yaml";
        let tekst = gebundeld()[awb].clone();
        bron.bestanden
            .insert("regulation/nl/wet/awb_kopie/1994-01-01.yaml".into(), tekst);
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(alle.iter().any(|c| c.casus.id == "voorbeeld_subsidie"));
        assert_eq!(standen[0].meldingen.len(), 1, "{:?}", standen[0]);
        assert!(
            standen[0].meldingen[0].contains("bron/voorbeeld/regulation/nl/wet/awb_kopie"),
            "{:?}",
            standen[0]
        );
    }

    #[test]
    fn a_cell_of_the_source_with_a_regelrecht_name_is_set_aside_with_a_message() {
        let mut bron = voorbeeld();
        bron.bestanden.insert(
            "cells/obs_de_klimop/cel.yaml".into(),
            "name: Een andere school\n".into(),
        );
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(
            standen[0]
                .meldingen
                .iter()
                .any(|m| m.contains("cell 'obs_de_klimop'")),
            "{:?}",
            standen[0]
        );
    }

    #[test]
    fn the_same_id_twice_in_one_source_is_an_error() {
        let mut bron = voorbeeld();
        let pad = "regulation/nl/uitvoeringsbeleid/voorbeeld_bron_subsidie/2026-01-01.yaml";
        let tekst = bron.bestanden[pad].clone();
        bron.bestanden.insert(
            "regulation/nl/uitvoeringsbeleid/tweede_kopie/2026-01-01.yaml".into(),
            tekst,
        );
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        let fout = standen[0].fout.clone().unwrap();
        assert!(
            fout.contains("'voorbeeld_bron_subsidie' is in both"),
            "{fout}"
        );
    }

    #[test]
    fn a_case_from_a_source_with_the_id_of_a_case_before_it_is_set_aside() {
        let mut bron = voorbeeld();
        let casus = bron.bestanden["casussen/voorbeeld_subsidie.yaml"].clone();
        bron.bestanden
            .insert("casussen/vakantieverlof.yaml".into(), casus);
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        let verlof: Vec<_> = alle
            .iter()
            .filter(|c| c.casus.id == "vakantieverlof")
            .collect();
        assert_eq!(verlof.len(), 1);
        assert_eq!(verlof[0].casus.bron, None);
        assert!(
            standen[0].meldingen[0].contains("'vakantieverlof'"),
            "{:?}",
            standen[0]
        );
    }

    /// The bundle's error when `files` replaces the file at `van` with the
    /// file at `naar`, its text changed by `wijzig`.
    fn verplaatst(van: &str, naar: &str, wijzig: impl Fn(&str) -> String) -> String {
        let mut files = gebundeld();
        let tekst = files.remove(van).unwrap();
        files.insert(naar.into(), wijzig(&tekst));
        casussen(files).unwrap_err().to_string()
    }

    const VERLOF: &str =
        "corpus/cells/obs_de_klimop/reductions/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml";

    #[test]
    fn a_reduction_under_regulation_is_refused() {
        let e = verplaatst(
            VERLOF,
            "corpus/regulation/nl/reducties/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml",
            str::to_string,
        );
        assert!(e.contains("a REDUCTIE is no regulation"), "{e}");
    }

    #[test]
    fn a_reduction_in_a_directory_not_named_by_its_id_is_refused() {
        let e = verplaatst(
            VERLOF,
            "corpus/cells/obs_de_klimop/reductions/al_verleend/2026-08-01.yaml",
            str::to_string,
        );
        assert!(e.contains("a directory named by its $id"), "{e}");
    }

    #[test]
    fn a_directory_of_reductions_holds_only_reductions() {
        let mut files = gebundeld();
        let beleid = files
            .keys()
            .find(|p| p.contains("fictief_beleid_vestiging_obs_de_klimop/"))
            .unwrap()
            .clone();
        let tekst = files[&beleid].clone();
        files.insert(
            "corpus/cells/reductions/fictief_beleid_vestiging_obs_de_klimop/2026-08-01.yaml".into(),
            tekst,
        );
        files.remove(&beleid);
        let e = casussen(files).unwrap_err().to_string();
        assert!(e.contains("holds only REDUCTIE"), "{e}");
    }

    #[test]
    fn a_reduction_that_reads_one_of_another_cell_is_refused() {
        // The school's reduction asks the CBR's: that one would read the
        // school's chronicle, not the CBR's.
        let e = verplaatst(VERLOF, VERLOF, |tekst| {
            tekst.replacen(
                "regulation: reductie_algemene_wet_bestuursrecht",
                "regulation: reductie_eerder_verstrekte_gehandicaptenparkeerkaart_cbr",
                1,
            )
        });
        assert!(
            e.contains("of the cell 'cbr'") && e.contains("only those of its own cell"),
            "{e}"
        );
    }
}
