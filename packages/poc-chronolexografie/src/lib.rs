//! PoC: een uitvoeringsproces naspelen op de cel van een fictieve uitvoerder
//! (chronolexografie, RFC-045 tot en met RFC-050). De bezoeker kiest een
//! casus: vakantieverlof (Leerplichtwet 1969 art. 13a) bij OBS De Klimop, de
//! gehandicaptenparkeerkaart (BABW art. 49) bij de gemeente Wilgendaal, of de
//! bedrijfsparkeervergunning (een fictieve parkeerverordening, art. 2) bij
//! diezelfde gemeente.
//!
//! De cel zelf staat in `packages/cel`; deze crate zet er het spel van één
//! bezoeker omheen ([`spel`]) en voegt niets toe aan wat de cel doet
//! ([`kern`]). Een casus noemt alleen de cel en het artikel dat de aanvraag
//! vestigt; de rest leidt [`casus`] af uit de wet.
//!
//! Het spel draait in de browser van de bezoeker, als WebAssembly
//! ([`wasm`], feature `wasm`), op de bestanden die de pagina meekrijgt
//! ([`bundel`]). Er is geen server met gedeelde toestand: elk tabblad speelt
//! zijn eigen casus.

pub mod bundel;
pub mod casus;
pub mod kern;
pub mod spel;
#[cfg(feature = "wasm")]
pub mod wasm;
