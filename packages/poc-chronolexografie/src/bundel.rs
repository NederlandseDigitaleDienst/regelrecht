//! The bundle the app plays from in the browser: the files of this crate's
//! own corpus ([`CORPUS`]), by their path in the repository, built into the
//! page.
//!
//! That corpus has the layout of a source: the cases (`casussen/`), the
//! regelwerken they run (`regulation/nl/`) and the cells (`cells/`, with
//! their reductions). It holds copies of the laws the cases run, the Awb
//! among them, so work on the cases never changes the regelrecht corpus and
//! the other way round. The register (`pocs/registry.yaml`, `corpus` of
//! `chronolexografie`) names the directory for the build, which copies
//! every file under it except the scenarios. A test below holds that every
//! directory in it is one a case needs ([`mappen`]), so nothing dead ends up
//! in the browser.
//!
//! Next to it the page may hold sources ([`Bron`]): a corpus the portal
//! fetches when the page loads, such as a branch of another repository
//! (`sources` of `chronolexografie` in the register). A source holds its own
//! cases (`casussen/`), regelwerken (`regulation/nl/`) and cells (`cells/`,
//! with their reductions).
//! A case from a source runs on that source alone: one corpus at a time,
//! so its own Awb and regelwerken are the ones that count, and nothing of
//! this crate's corpus takes their place. The cases of this crate never
//! see a source ([`met_bronnen`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_cel::{Bundle, Error};
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::casus::Casus;
use crate::casus::{casussen as lees_casussen, Regelwerken};
use crate::kern::{Corpus, Result};

/// This crate's corpus, under the repository root: the cases under
/// `casussen`, the regelwerken under `regulation/nl` and the cells under
/// `cells`, with their reductions.
pub const CORPUS: &str = "packages/poc-chronolexografie/corpus";

/// The case files, under the repository root.
pub const CASUSSEN: &str = "packages/poc-chronolexografie/corpus/casussen";

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
/// `bron/<name>/...`.
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
/// (`fouten`), and what loading set aside (`meldingen`), such as a case
/// with the id of a case before it.
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
/// `bronnen`, each on its own source alone. A broken
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
            match uit_bron(&bron, &mut stand) {
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

/// The cases of `bron`, each on the source under `bron/<name>` alone. A case
/// that does not load is in `stand.fouten`.
fn uit_bron(bron: &Bron, stand: &mut BronStand) -> Result<Vec<Corpus>> {
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
    let alles: BTreeMap<String, String> = bron
        .bestanden
        .iter()
        .map(|(pad, tekst)| (format!("{}/{pad}", wortel.display()), tekst.clone()))
        .collect();
    let bundle: Arc<Bundle> = Arc::new(
        Bundle::new(alles).map_err(|e| Error::Setup(format!("source '{}': {e}", bron.naam)))?,
    );
    let roots = [wortel.clone()];
    let regelwerken = Regelwerken::load(&*bundle, &roots)?;
    stand
        .meldingen
        .extend(regelwerken.meldingen.iter().cloned());
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
/// holds: the case files, and per case its corpus ([`Casus::corpus`]) and
/// the directories of its own cell and the other cells of its sphere
/// (RFC-052).
pub fn mappen(alle: &[Corpus]) -> BTreeSet<String> {
    let mut uit = BTreeSet::from([CASUSSEN.to_string()]);
    for c in alle {
        uit.extend(c.casus.corpus.iter().map(|m| format!("{CORPUS}/{m}")));
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
    fn the_register_names_this_crates_corpus() {
        assert_eq!(in_het_register(), BTreeSet::from([CORPUS.to_string()]));
    }

    #[test]
    fn every_directory_of_the_corpus_is_one_a_case_needs() {
        // The build bundles the whole directory, so a regelwerk or a cell
        // that no case runs would go to the browser for nothing.
        let alle = Corpus::alle(&repo().join(CORPUS), &repo().join(CASUSSEN)).unwrap();
        let nodig = mappen(&alle);
        let aanwezig: BTreeSet<String> = gebundeld()
            .keys()
            .map(|pad| pad.rsplit_once('/').unwrap().0.to_string())
            .collect();
        for map in aanwezig {
            assert!(
                nodig
                    .iter()
                    .any(|n| map == *n || map.starts_with(&format!("{n}/"))),
                "{map} is in the corpus, but no case needs it"
            );
        }
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
    fn a_bundle_without_a_file_a_case_needs_fails_at_the_start() {
        let mut files = BTreeMap::new();
        for dir in in_het_register() {
            verzamel(Path::new(&dir), &mut files);
        }
        // Without its catalogue the school has no cell.
        let mut without_cell = files.clone();
        without_cell.remove(&format!("{CORPUS}/cells/obs_de_klimop/cel.yaml"));
        let e = casussen(without_cell).unwrap_err().to_string();
        assert!(e.contains(&format!("{CORPUS}/cells/obs_de_klimop")), "{e}");
        // Without a reduction its catalogue names, the cell does not open.
        files.remove(&format!("{CORPUS}/{VERLOF}"));
        let e = casussen(files).unwrap_err().to_string();
        assert!(
            e.contains("reductie_verlof_al_verleend_in_schooljaar")
                && e.contains("only the answers of its own reductions"),
            "{e}"
        );
    }

    /// The own regelwerk of the case of the parking card, by its path under
    /// [`CORPUS`].
    const GGD_BELEID: &str = "casussen/gehandicaptenparkeerkaart/regulation/nl/uitvoeringsbeleid/\
         beleid_geneeskundig_advies_ggd_wilgendaal/2026-01-01.yaml";

    #[test]
    fn the_own_regelwerken_of_a_case_count_without_being_named() {
        // Moved to the shared regulation and named by no case, the policy
        // counts for none, and the case that needs it no longer loads.
        let e = verplaatst(
            GGD_BELEID,
            "regulation/nl/uitvoeringsbeleid/beleid_geneeskundig_advies_ggd_wilgendaal/2026-01-01.yaml",
            str::to_string,
        );
        assert!(e.contains("gehandicaptenparkeerkaart"), "{e}");
    }

    #[test]
    fn a_case_that_names_the_own_regelwerken_of_another_is_refused() {
        let mut files = gebundeld();
        let pad = format!("{CASUSSEN}/bedrijfsparkeervergunning.yaml");
        let tekst = files[&pad].replacen(
            "corpus:\n",
            "corpus:\n  - casussen/gehandicaptenparkeerkaart/regulation/nl/uitvoeringsbeleid/beleid_geneeskundig_advies_ggd_wilgendaal\n",
            1,
        );
        files.insert(pad, tekst);
        let e = casussen(files).unwrap_err().to_string();
        assert!(e.contains("count only for that case"), "{e}");
    }

    #[test]
    fn own_regelwerken_without_a_case_are_refused() {
        let e = verplaatst(
            GGD_BELEID,
            "casussen/los/regulation/nl/uitvoeringsbeleid/beleid_geneeskundig_advies_ggd_wilgendaal/2026-01-01.yaml",
            str::to_string,
        );
        assert!(e.contains("no case los.yaml"), "{e}");
    }

    /// The bundle of this crate as the build makes it.
    fn gebundeld() -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        for dir in in_het_register() {
            verzamel(Path::new(&dir), &mut files);
        }
        files
    }

    /// A source on disk as the portal gives it: a folder, or a manifest
    /// (packages/poc-portal/src/bron.rs).
    fn uit_map(pad: &Path) -> BTreeMap<String, String> {
        regelrecht_poc_portal::bron::lokaal(pad).unwrap()
    }

    /// The Awb of this crate's corpus and the reductions of no cell, by
    /// their path in a source.
    fn awb() -> BTreeMap<String, String> {
        gebundeld()
            .into_iter()
            .filter_map(|(pad, tekst)| {
                let pad = pad.strip_prefix(&format!("{CORPUS}/"))?.to_string();
                (pad.starts_with("regulation/nl/wet/algemene_wet_bestuursrecht/")
                    || pad.starts_with("cells/reductions/"))
                .then_some((pad, tekst))
            })
            .collect()
    }

    /// The test source in `tests/bron-voorbeeld`, with a copy of the Awb: a
    /// source brings every regelwerk its cases run.
    fn voorbeeld() -> Bron {
        let mut bestanden =
            uit_map(&repo().join("packages/poc-chronolexografie/tests/bron-voorbeeld"));
        bestanden.extend(awb());
        Bron {
            naam: "voorbeeld".into(),
            versie: Some("abc1234".into()),
            bestanden,
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

    /// With `POC_BRON_NAPP` set to a local source folder or manifest (as for `just
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
    fn a_case_from_a_source_plays_on_its_source_alone() {
        let (alle, standen) = met_bronnen(gebundeld(), vec![voorbeeld()]).unwrap();
        let c = alle
            .iter()
            .find(|c| c.casus.id == "voorbeeld_subsidie")
            .expect("the case of the source");
        assert_eq!(c.casus.bron.as_deref(), Some("voorbeeld"));
        // The regulation, the cell and the objection from the source.
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

    /// The test regulation with the application in article 1 and the
    /// decision it asks for in article 2 (`requests`, RFC-047).
    const AANVRAAG_EN_BESLUIT_APART: &str = r#"---
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.8/schema/v0.8.8/schema.json
$id: voorbeeld_bron_subsidie
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.invalid/voorbeeld_bron_subsidie
name: Voorbeeldsubsidie (fictieve testbron)
competent_authority:
  name: Voorbeeldautoriteit
articles:
  - number: '1'
    text: FICTIEF. Een vereniging kan subsidie aanvragen voor een subsidiejaar.
    url: https://example.invalid/voorbeeld_bron_subsidie#1
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: voorbeeld_bron_subsidie#2
            parameters:
              - naam_partij
              - subsidiejaar
            outputs:
              - aanvraag_ontvangen
        parameters:
          - name: naam_partij
            type: string
            description: De naam van de vereniging die subsidie aanvraagt.
          - name: subsidiejaar
            type: number
            description: Het jaar waarvoor subsidie wordt gevraagd.
        output:
          - name: aanvraag_ontvangen
            type: boolean
            description: Dat de aanvraag is ontvangen.
        actions:
          - output: aanvraag_ontvangen
            value: true
  - number: '2'
    text: >-
      FICTIEF. De Voorbeeldautoriteit besluit op de aanvraag, en verleent de
      subsidie indien de vereniging geregistreerd is.
    url: https://example.invalid/voorbeeld_bron_subsidie#2
    machine_readable:
      open_terms:
        - id: partij_geregistreerd
          type: boolean
          required: true
          decided_per_case_by: de Voorbeeldautoriteit (artikel 2)
          description: Of de aanvragende vereniging geregistreerd is.
      execution:
        produces:
          - type: BESCHIKKING
            decision_type: TOEKENNING
            outputs:
              - subsidie_verleend
        parameters:
          - name: naam_partij
            type: string
            description: De naam van de vereniging die subsidie aanvraagt.
          - name: subsidiejaar
            type: number
            description: Het jaar waarvoor subsidie wordt gevraagd.
        output:
          - name: subsidie_verleend
            type: boolean
            description: Of de autoriteit de subsidie verleent.
        actions:
          - output: subsidie_verleend
            value: $partij_geregistreerd
"#;

    /// The test regulation of the source `voorbeeld`.
    const VOORBEELD_WET: &str =
        "regulation/nl/uitvoeringsbeleid/voorbeeld_bron_subsidie/2026-01-01.yaml";

    /// The source `voorbeeld` with its test regulation replaced by `wet`.
    fn voorbeeld_met(wet: &str) -> Bron {
        let mut bron = voorbeeld();
        bron.bestanden.insert(VOORBEELD_WET.into(), wet.into());
        bron
    }

    #[test]
    fn a_case_decides_on_the_article_its_application_asks_for() {
        let bron = voorbeeld_met(AANVRAAG_EN_BESLUIT_APART);
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(standen[0].fouten.is_empty(), "{:?}", standen[0]);
        let c = alle
            .iter()
            .find(|c| c.casus.id == "voorbeeld_subsidie")
            .unwrap();
        assert_eq!(
            (c.afleiding.article.as_str(), c.afleiding.decision.as_str()),
            ("1", "2")
        );
        let mut spel = crate::spel::Spel::met_bronnen(alle, standen);
        let outputs = speel(&mut spel, "voorbeeld_subsidie");
        assert_eq!(outputs["subsidie_verleend"], serde_json::json!(true));
    }

    #[test]
    fn the_authority_gives_what_the_decision_asks_beside_its_findings() {
        let mut bron = voorbeeld();
        let tekst = AANVRAAG_EN_BESLUIT_APART.replace(
            "        output:\n          - name: subsidie_verleend",
            "          - name: motivering\n            type: string\n            \
             description: Waarom de autoriteit zo besluit.\n        output:\n          \
             - name: subsidie_verleend",
        );
        assert_ne!(tekst, AANVRAAG_EN_BESLUIT_APART);
        bron.bestanden.insert(
            "regulation/nl/uitvoeringsbeleid/voorbeeld_bron_subsidie/2026-01-01.yaml".into(),
            tekst,
        );
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(standen[0].fouten.is_empty(), "{:?}", standen[0]);
        let mut spel = crate::spel::Spel::met_bronnen(alle, standen);
        let casus = spel.kiezen("voorbeeld_subsidie").unwrap();
        let dag = casus["speeldatum"].as_str().unwrap().parse().unwrap();
        let vormen = spel.vormen(dag).unwrap();
        let gevraagd: Vec<&str> = vormen["besluit"]["bevindingen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["naam"].as_str().unwrap())
            .collect();
        assert_eq!(gevraagd, ["partij_geregistreerd", "motivering"]);
        let aanvraag = casus["voorbeeld"]["aanvraag"].as_object().cloned().unwrap();
        let gram = spel.aanvragen(&aanvraag, dag).unwrap();
        let gegeven =
            serde_json::json!({"partij_geregistreerd": true, "motivering": "geregistreerd"});
        let besluit = spel
            .besluiten(
                gram["id"].as_str().unwrap(),
                gegeven.as_object().unwrap(),
                dag,
            )
            .unwrap();
        assert_eq!(besluit["besluit"]["fields"]["motivering"], "geregistreerd");
        assert_eq!(besluit["besluit"]["outputs"]["subsidie_verleend"], true);
    }

    #[test]
    fn an_application_that_asks_for_a_missing_article_is_named() {
        let wet = AANVRAAG_EN_BESLUIT_APART.replace(
            "requests: voorbeeld_bron_subsidie#2",
            "requests: voorbeeld_bron_subsidie#9",
        );
        let fout = fouten_met(&wet, "artikel: voorbeeld_bron_subsidie#1");
        assert!(
            fout.contains("asks for the decision of voorbeeld_bron_subsidie#9"),
            "{fout}"
        );
    }

    /// The errors of the source `voorbeeld` with its test regulation
    /// replaced by `wet` and the case's `artikel:` line by `artikel`.
    fn fouten_met(wet: &str, artikel: &str) -> String {
        let mut bron = voorbeeld_met(wet);
        let casus = "casussen/voorbeeld_subsidie.yaml";
        let tekst = bron.bestanden[casus].replace("artikel: voorbeeld_bron_subsidie#1", artikel);
        bron.bestanden.insert(casus.into(), tekst);
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        standen[0].fouten.join("\n")
    }

    #[test]
    fn an_application_that_asks_for_an_article_without_a_decision_is_named() {
        let wet = AANVRAAG_EN_BESLUIT_APART.replace(
            "requests: voorbeeld_bron_subsidie#2",
            "requests: voorbeeld_bron_subsidie#1",
        );
        let fout = fouten_met(&wet, "artikel: voorbeeld_bron_subsidie#1");
        assert!(
            fout.contains(
                "asks for the decision of voorbeeld_bron_subsidie#1, which its corpus does not \
                 hold as an article that takes a decision"
            ),
            "{fout}"
        );
    }

    #[test]
    fn a_case_on_a_missing_law_or_article_says_which() {
        let fout = fouten_met(
            AANVRAAG_EN_BESLUIT_APART,
            "artikel: voorbeeld_bron_subsidie#9",
        );
        assert!(
            fout.contains("'voorbeeld_bron_subsidie' has no article 9"),
            "{fout}"
        );
        let fout = fouten_met(AANVRAAG_EN_BESLUIT_APART, "artikel: niet_in_het_corpus#1");
        assert!(
            fout.contains("needs regelwerk 'niet_in_het_corpus', which is not in the corpus"),
            "{fout}"
        );
    }

    #[test]
    fn who_decides_follows_from_the_article_that_takes_the_decision() {
        let wet = AANVRAAG_EN_BESLUIT_APART.replace(
            "    machine_readable:\n      open_terms:",
            "    machine_readable:\n      competent_authority:\n        name: Tweede autoriteit\n      open_terms:",
        );
        let bron = voorbeeld_met(&wet);
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(standen[0].fouten.is_empty(), "{:?}", standen[0]);
        let c = alle
            .iter()
            .find(|c| c.casus.id == "voorbeeld_subsidie")
            .unwrap();
        assert_eq!(c.afleiding.rol, "Tweede autoriteit");
    }

    #[test]
    fn a_source_without_the_awb_does_not_get_the_one_of_this_crate() {
        let mut bron = voorbeeld();
        bron.bestanden.retain(|pad, _| !awb().contains_key(pad));
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(alle.iter().all(|c| c.casus.bron.is_none()));
        assert!(standen[0].casussen.is_empty(), "{:?}", standen[0]);
    }

    #[test]
    fn a_source_runs_its_own_copy_of_an_id_of_this_crate() {
        let mut bron = voorbeeld();
        let awb = "regulation/nl/wet/algemene_wet_bestuursrecht/1994-01-01.yaml";
        let tekst = bron.bestanden.remove(awb).unwrap();
        bron.bestanden
            .insert("regulation/nl/wet/awb_kopie/1994-01-01.yaml".into(), tekst);
        let casus = "casussen/voorbeeld_subsidie.yaml";
        let tekst = bron.bestanden[casus].replace(
            "regulation/nl/wet/algemene_wet_bestuursrecht",
            "regulation/nl/wet/awb_kopie",
        );
        bron.bestanden.insert(casus.into(), tekst);
        let (alle, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(alle.iter().any(|c| c.casus.id == "voorbeeld_subsidie"));
        assert!(standen[0].meldingen.is_empty(), "{:?}", standen[0]);
    }

    /// The case `voorbeeld_subsidie` of the test source with `wijzig`
    /// applied to its file: why it did not load.
    fn fout_van_voorbeeld(wijzig: impl Fn(&str) -> String) -> String {
        let mut bron = voorbeeld();
        let casus = "casussen/voorbeeld_subsidie.yaml";
        let tekst = wijzig(&bron.bestanden[casus]);
        bron.bestanden.insert(casus.into(), tekst);
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(standen[0].casussen.is_empty(), "{:?}", standen[0]);
        standen[0].fouten.join("\n")
    }

    #[test]
    fn a_regelwerk_outside_the_corpus_of_a_case_does_not_count() {
        // The Awb is in the source, but not in the corpus of the case.
        let fout = fout_van_voorbeeld(|t| {
            t.replace("  - regulation/nl/wet/algemene_wet_bestuursrecht\n", "")
        });
        assert!(fout.contains("BEZWAAR"), "{fout}");
    }

    #[test]
    fn an_entry_of_the_corpus_without_a_regelwerk_is_named() {
        let fout = fout_van_voorbeeld(|t| {
            t.replace("corpus:\n", "corpus:\n  - regulation/nl/wet/bestaat_niet\n")
        });
        assert!(
            fout.contains("names 'regulation/nl/wet/bestaat_niet', which holds no regelwerk"),
            "{fout}"
        );
    }

    #[test]
    fn a_cell_of_the_source_with_a_name_of_this_crate_is_its_own() {
        let mut bron = voorbeeld();
        bron.bestanden.insert(
            "cells/obs_de_klimop/cel.yaml".into(),
            "name: Een andere school\n".into(),
        );
        let (_, standen) = met_bronnen(gebundeld(), vec![bron]).unwrap();
        assert!(standen[0].meldingen.is_empty(), "{:?}", standen[0]);
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
    /// file at `naar` (both under [`CORPUS`]), its text changed by `wijzig`.
    fn verplaatst(van: &str, naar: &str, wijzig: impl Fn(&str) -> String) -> String {
        let mut files = gebundeld();
        let tekst = files.remove(&format!("{CORPUS}/{van}")).unwrap();
        files.insert(format!("{CORPUS}/{naar}"), wijzig(&tekst));
        casussen(files).unwrap_err().to_string()
    }

    /// A reduction of the school, by its path under [`CORPUS`].
    const VERLOF: &str =
        "cells/obs_de_klimop/reductions/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml";

    #[test]
    fn a_reduction_under_regulation_is_refused() {
        let e = verplaatst(
            VERLOF,
            "regulation/nl/reducties/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml",
            str::to_string,
        );
        assert!(e.contains("a REDUCTIE is no regulation"), "{e}");
    }

    #[test]
    fn a_reduction_in_a_directory_not_named_by_its_id_is_refused() {
        let e = verplaatst(
            VERLOF,
            "cells/obs_de_klimop/reductions/al_verleend/2026-08-01.yaml",
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
            format!(
                "{CORPUS}/cells/reductions/fictief_beleid_vestiging_obs_de_klimop/2026-08-01.yaml"
            ),
            tekst,
        );
        files.remove(&beleid);
        let e = casussen(files).unwrap_err().to_string();
        assert!(e.contains("holds only REDUCTIE"), "{e}");
    }

    #[test]
    fn a_loose_file_beside_the_reductions_of_no_cell_is_refused() {
        let mut files = gebundeld();
        files.insert(format!("{CORPUS}/cells/reductions/los.yaml"), String::new());
        let e = casussen(files).unwrap_err().to_string();
        assert!(e.contains("a file per version"), "{e}");
    }

    #[test]
    fn a_reduction_that_reads_one_of_another_cell_is_refused() {
        // The school's reduction asks the Chamber's: that one would read the
        // school's chronicle, not the Chamber's.
        let e = verplaatst(VERLOF, VERLOF, |tekst| {
            tekst.replacen(
                "regulation: reductie_algemene_wet_bestuursrecht",
                "regulation: reductie_vestiging_in_handelsregister",
                1,
            )
        });
        assert!(
            e.contains("of the cell 'kamer_van_koophandel'")
                && e.contains("only those of its own cell"),
            "{e}"
        );
    }
}
