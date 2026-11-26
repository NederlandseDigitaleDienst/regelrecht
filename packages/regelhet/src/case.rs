//! The one case this server runs, from the source manifest that ships with
//! it.
//!
//! A manifest may make several cases, one per case file under `casussen/`,
//! so that one repository holds several processes. The deployment then
//! chooses one by its id, the name of its case file
//! (`REGELHET_CASUS`, [`crate::config`]); a manifest of one case needs no
//! choice. Every case of the manifest still has to load: the manifest is one
//! source, and a source with a broken case is not one to run from.
//!
//! The manifest is read as the poc-portal reads one
//! (`regelrecht_corpus::source_manifest::lokaal`), and the source it makes is
//! loaded as the chronolexografie poc loads a source
//! (`regelrecht_poc_chronolexografie::bundel::uit_bron`). Where the poc
//! plays on when a case of a source does not load, this server does not
//! start: a process that half loads is no process to run.

use std::collections::BTreeMap;
use std::path::Path;

use regelrecht_poc_chronolexografie::bundel::{self, Bron, BronStand};
use regelrecht_poc_chronolexografie::kern::Corpus;

/// The name of the source in the bundle the case is loaded from.
const SOURCE: &str = "regelhet";

/// Why the manifest gives no case to run.
#[derive(Debug, thiserror::Error)]
pub enum CaseError {
    #[error("the manifest does not read: {0}")]
    Manifest(String),
    #[error("the source does not load: {0}")]
    Source(#[from] regelrecht_cel::Error),
    #[error("a case does not load: {}", .0.join("; "))]
    Cases(Vec<String>),
    #[error("the manifest makes no case")]
    NoCase,
    #[error(
        "the manifest makes {} cases ({}); choose one with REGELHET_CASUS",
        .0.len(),
        .0.join(", ")
    )]
    NoChoice(Vec<String>),
    #[error("REGELHET_CASUS is `{chosen}`, which the manifest does not make; it makes {}", .ids.join(", "))]
    UnknownCase { chosen: String, ids: Vec<String> },
}

/// The case in the source the manifest at `manifest` makes, when it makes
/// exactly one, with the messages loading it gave, for the log.
///
/// # Errors
///
/// As [`load_chosen`] without a choice.
pub fn load(manifest: &Path) -> Result<(Corpus, Vec<String>), CaseError> {
    load_chosen(manifest, None)
}

/// The case `chosen` (the name of its case file) in the source the manifest
/// at `manifest` makes, or without a choice its only case, with the messages
/// loading it gave (such as a regelwerk version set aside), for the log.
///
/// # Errors
///
/// When the manifest does not read, the source does not load, a case in it
/// does not load, or it holds no case; without a choice, when it holds more
/// than one; with one, when none has that id.
pub fn load_chosen(
    manifest: &Path,
    chosen: Option<&str>,
) -> Result<(Corpus, Vec<String>), CaseError> {
    from_files(files(manifest)?, chosen)
}

/// The files of the source the manifest at `manifest` makes, by their path
/// in the source: what [`from_files`] loads, and what the FSC voor Burgers
/// channel hands an app to compute on (`crate::fsc4b::REGELWERKEN`).
///
/// # Errors
///
/// When the manifest or a file it names does not read.
pub fn files(manifest: &Path) -> Result<BTreeMap<String, String>, CaseError> {
    regelrecht_corpus::source_manifest::lokaal(manifest).map_err(CaseError::Manifest)
}

/// The case `chosen` in the source of `files` ([`files`]), or without a
/// choice its only case, as [`load_chosen`] gives it.
///
/// # Errors
///
/// As [`load_chosen`], except that the manifest is read already.
pub fn from_files(
    files: BTreeMap<String, String>,
    chosen: Option<&str>,
) -> Result<(Corpus, Vec<String>), CaseError> {
    let bron = Bron {
        naam: SOURCE.to_string(),
        bestanden: files,
        ..Bron::default()
    };
    let mut stand = BronStand::default();
    let cases = bundel::uit_bron(&bron, &mut stand)?;
    if !stand.fouten.is_empty() {
        return Err(CaseError::Cases(stand.fouten));
    }
    let case = choose(cases, chosen)?;
    Ok((case, stand.meldingen))
}

/// The case of `cases` whose id is `chosen`, or without a choice the only
/// one.
fn choose(mut cases: Vec<Corpus>, chosen: Option<&str>) -> Result<Corpus, CaseError> {
    let ids = || cases_ids(&cases);
    match chosen {
        Some(id) => match cases.iter().position(|c| c.casus.id == id) {
            Some(i) => Ok(cases.swap_remove(i)),
            None => Err(CaseError::UnknownCase {
                chosen: id.to_string(),
                ids: ids(),
            }),
        },
        None if cases.len() > 1 => Err(CaseError::NoChoice(ids())),
        None => cases.pop().ok_or(CaseError::NoCase),
    }
}

/// The ids of `cases`, sorted, for a message.
fn cases_ids(cases: &[Corpus]) -> Vec<String> {
    let mut ids: Vec<String> = cases.iter().map(|c| c.casus.id.clone()).collect();
    ids.sort();
    ids
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_MANIFEST;

    #[test]
    fn the_manifest_of_this_crate_makes_the_parking_card_case() {
        let (corpus, _) = load(Path::new(DEFAULT_MANIFEST)).unwrap();
        assert_eq!(corpus.casus.id, "gehandicaptenparkeerkaart");
        assert_eq!(corpus.casus.cel, "gemeente_wilgendaal");
        let cellen: Vec<&str> = corpus.cellen().collect();
        assert_eq!(
            cellen,
            [
                "gemeente_wilgendaal",
                "ggd_wilgendaal",
                "rechtbank_wilgendaal"
            ]
        );
    }

    /// A name under `uitgeven` that is no answer of the case's cell stops
    /// the load: a typo would hand out nothing without saying so.
    #[test]
    fn uitgeven_names_only_answers_of_the_cell() {
        const CASE: &str = "casussen/gehandicaptenparkeerkaart.yaml";
        let base = files(Path::new(DEFAULT_MANIFEST)).unwrap();
        let (corpus, _) = from_files(base.clone(), None).unwrap();
        assert_eq!(
            corpus.casus.uitgeven,
            [
                "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt",
                "verzoek_om_geneeskundig_onderzoek_staat_open"
            ]
        );
        let mut files = base;
        let case = files.get_mut(CASE).unwrap();
        *case = case.replace(
            "  - verzoek_om_geneeskundig_onderzoek_staat_open\n",
            "  - geen_antwoord_van_de_cel\n",
        );
        let e = from_files(files, None).unwrap_err().to_string();
        assert!(e.contains("geen_antwoord_van_de_cel"), "{e}");
    }

    #[test]
    fn the_only_case_may_also_be_chosen_by_its_id() {
        let (corpus, _) = load_chosen(
            Path::new(DEFAULT_MANIFEST),
            Some("gehandicaptenparkeerkaart"),
        )
        .unwrap();
        assert_eq!(corpus.casus.id, "gehandicaptenparkeerkaart");
        let e = load_chosen(Path::new(DEFAULT_MANIFEST), Some("zorgtoeslag")).unwrap_err();
        assert!(
            matches!(&e, CaseError::UnknownCase { chosen, ids } if chosen == "zorgtoeslag" && ids == &["gehandicaptenparkeerkaart"]),
            "{e}"
        );
    }

    /// A manifest with every case of the chronolexografie poc.
    const SEVERAL: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/meerdere-casussen/bron.yaml"
    );

    #[test]
    fn a_manifest_of_several_cases_runs_the_one_chosen_and_refuses_no_choice() {
        let e = load(Path::new(SEVERAL)).unwrap_err();
        let CaseError::NoChoice(ids) = &e else {
            unreachable!("no choice among several: {e}");
        };
        assert!(ids.len() > 1, "{ids:?}");
        assert!(
            ids.iter().any(|i| i == "bedrijfsparkeervergunning"),
            "{ids:?}"
        );
        assert!(e.to_string().contains("REGELHET_CASUS"), "{e}");

        let (corpus, _) =
            load_chosen(Path::new(SEVERAL), Some("bedrijfsparkeervergunning")).unwrap();
        assert_eq!(corpus.casus.id, "bedrijfsparkeervergunning");
        assert_eq!(corpus.casus.titel, "Bedrijfsparkeervergunning");

        let e = load_chosen(Path::new(SEVERAL), Some("parkeren")).unwrap_err();
        assert!(matches!(e, CaseError::UnknownCase { .. }), "{e}");
    }

    #[test]
    fn a_manifest_that_does_not_read_is_an_error() {
        let e = load(Path::new("/nonexistent/bron.yaml")).unwrap_err();
        assert!(matches!(e, CaseError::Manifest(_)), "{e}");
    }
}
