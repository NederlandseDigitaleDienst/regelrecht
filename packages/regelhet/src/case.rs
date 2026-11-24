//! The one case this server runs, from the source manifest that ships with
//! it.
//!
//! The manifest is read as the poc-portal reads one
//! (`regelrecht_corpus::source_manifest::lokaal`), and the source it makes is
//! loaded as the chronolexografie poc loads a source
//! (`regelrecht_poc_chronolexografie::bundel::uit_bron`). Where the poc
//! plays on when a case of a source does not load, this server does not
//! start: a process that half loads is no process to run.

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
    #[error("the manifest makes {0} cases; this server runs exactly one")]
    NotOne(usize),
}

/// The case in the source the manifest at `manifest` makes, with the
/// messages loading it gave (such as a regelwerk version set aside), for the
/// log.
///
/// # Errors
///
/// When the manifest does not read, the source does not load, a case in it
/// does not load, or it holds no case or more than one.
pub fn load(manifest: &Path) -> Result<(Corpus, Vec<String>), CaseError> {
    let bestanden =
        regelrecht_corpus::source_manifest::lokaal(manifest).map_err(CaseError::Manifest)?;
    let bron = Bron {
        naam: SOURCE.to_string(),
        bestanden,
        ..Bron::default()
    };
    let mut stand = BronStand::default();
    let mut cases = bundel::uit_bron(&bron, &mut stand)?;
    if !stand.fouten.is_empty() {
        return Err(CaseError::Cases(stand.fouten));
    }
    let Some(case) = cases.pop() else {
        return Err(CaseError::NotOne(0));
    };
    if !cases.is_empty() {
        return Err(CaseError::NotOne(cases.len() + 1));
    }
    Ok((case, stand.meldingen))
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

    #[test]
    fn a_manifest_that_does_not_read_is_an_error() {
        let e = load(Path::new("/nonexistent/bron.yaml")).unwrap_err();
        assert!(matches!(e, CaseError::Manifest(_)), "{e}");
    }
}
