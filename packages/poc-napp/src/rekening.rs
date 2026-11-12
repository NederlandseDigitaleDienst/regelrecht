//! Bank account ("rekening") of the legal entity behind a party.
//!
//! Legal model: the subsidy is granted to the rechtspersoon (Wpp art. 27).
//! Branches of a centrally organized party have no legal personality, so
//! there is exactly ONE account per legal entity, and only the
//! signing-authorized board may submit or change it. Changing the account is
//! the single most fraud-sensitive moment in a subsidy process, which is why
//! a session with a limited branch machtiging is refused here.
//!
//! Banks publish no account registers: the IBAN is the party's own
//! statement, guarded by two checks:
//!  1. real ISO 13616 mod-97 validation of the IBAN itself;
//!  2. a MOCKED IBAN-name check (SurePay-like, as used by the Dutch
//!     government): the submitted tenaamstelling is compared with the
//!     registered party name. The real service asks the bank; this demo
//!     only has the register to compare against.
//!
//! Legal entities not (yet) in the register cannot store an account: the
//! claim flow (parallel branch) creates the registration first, after which
//! the board can submit the account here. No shadow table.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

use crate::handlers::{ApiError, bad_request, forbidden_with, internal_error, session_kvk};
use crate::machtiging::{self, Machtiging};
use crate::register;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// IBAN validation (ISO 13616, mod 97-10)
// ---------------------------------------------------------------------------

/// Normalize an IBAN: strip whitespace, uppercase.
pub fn normalize_iban(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

/// Validate an IBAN per ISO 13616: normalize, check the structure (two
/// letters, two digits, 15-34 alphanumeric total), move the first four
/// characters to the end, replace letters by 10..35 and require the
/// resulting number to be ≡ 1 (mod 97). Returns the normalized IBAN or a
/// Dutch user-facing error message.
pub fn validate_iban(input: &str) -> Result<String, String> {
    let iban = normalize_iban(input);
    if iban.is_empty() {
        return Err("Vul een IBAN in.".to_string());
    }
    if !(15..=34).contains(&iban.len())
        || !iban.chars().all(|c| c.is_ascii_alphanumeric())
        || !iban.chars().take(2).all(|c| c.is_ascii_uppercase())
        || !iban.chars().skip(2).take(2).all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "'{input}' is geen geldig IBAN: een IBAN begint met een landcode en twee controlecijfers."
        ));
    }
    // Rearrange and compute mod 97 incrementally (the number exceeds u64).
    let rearranged = iban.chars().skip(4).chain(iban.chars().take(4));
    let mut remainder: u64 = 0;
    for c in rearranged {
        // De structuurcontrole hierboven heeft al vastgesteld dat elk teken
        // alfanumeriek is; to_digit(36) kan hier dus niet falen.
        #[allow(clippy::expect_used)]
        let value = c.to_digit(36).expect("alphanumeric after structure check") as u64;
        remainder = if value < 10 {
            (remainder * 10 + value) % 97
        } else {
            (remainder * 100 + value) % 97
        };
    }
    if remainder != 1 {
        return Err(format!(
            "'{input}' is geen geldig IBAN: de controlecijfers kloppen niet."
        ));
    }
    Ok(iban)
}

// ---------------------------------------------------------------------------
// MOCK IBAN-name check (SurePay-like)
// ---------------------------------------------------------------------------

/// Dutch stopwords and legal-form noise that carry no identity.
const NOISE_WORDS: &[&str] = &[
    "de",
    "het",
    "een",
    "van",
    "voor",
    "en",
    "der",
    "den",
    "vereniging",
    "stichting",
    "partij",
];

/// Normalize a name to comparison tokens: lowercase, punctuation removed.
/// Noise words are dropped unless that would leave nothing.
fn name_tokens(name: &str) -> Vec<String> {
    let all: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    let without_noise: Vec<String> = all
        .iter()
        .filter(|w| !NOISE_WORDS.contains(&w.as_str()))
        .cloned()
        .collect();
    if without_noise.is_empty() {
        all
    } else {
        without_noise
    }
}

/// MOCK IBAN-name check, as a pure fact producer. The real check (SurePay)
/// asks the bank whether the IBAN is held under this name; this demo
/// compares the submitted tenaamstelling with the registered party
/// aanduiding instead: normalized word overlap, where at least half of the
/// words of the shorter name must occur in the other. Without a registered
/// name there is nothing to compare. The verdict on this fact is not drawn
/// here: art. 27 Wpp (engine) decides whether the rekening is acceptable.
pub fn naam_komt_overeen(submitted: &str, registered_name: Option<&str>) -> bool {
    let Some(registered) = registered_name else {
        return true;
    };
    let submitted_tokens = name_tokens(submitted);
    let registered_tokens = name_tokens(registered);
    let overlap = submitted_tokens
        .iter()
        .filter(|t| registered_tokens.contains(t))
        .count();
    let shortest = submitted_tokens.len().min(registered_tokens.len()).max(1);
    overlap * 2 >= shortest
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/mijn-rekening — the account of the logged-in legal entity, or
/// nulls when none is known (or the organization is not in the register).
pub async fn get_mijn_rekening(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden_with("Geen toegang."));
    };
    let partij = register::partij_by_kvk(&state.pool, &kvk)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!({
        "iban": partij.as_ref().and_then(|p| p.iban.clone()),
        "tenaamstelling": partij.as_ref().and_then(|p| p.iban_tenaamstelling.clone()),
        "in_register": partij.is_some(),
    })))
}

#[derive(Deserialize)]
pub struct RekeningOpgave {
    pub iban: String,
    pub tenaamstelling: String,
}

/// PUT /api/mijn-rekening — submit or change the account of the legal
/// entity. The orchestration gathers the facts (machtiging, name check,
/// registration); art. 27 Wpp draws the verdicts: only the
/// signing-authorized board may change the account (the fraud-sensitive
/// moment) and the account must be held in the name of the rechtspersoon.
pub async fn put_mijn_rekening(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<RekeningOpgave>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden_with("Geen toegang."));
    };
    let partij = register::partij_by_kvk(&state.pool, &kvk)
        .await
        .map_err(internal_error)?;
    // Not in the register: no shadow administration. The claim flow
    // (parallel branch) creates the registration; until then there is no
    // registered legal entity to attach an account to.
    let Some(partij) = partij else {
        return Err(bad_request(
            "Uw organisatie staat nog niet in het partijregister. Zodra de registratie is \
             vastgelegd kan het bestuur hier een rekeningnummer opgeven.",
        ));
    };
    // Technical input validation (no legal content): ISO 13616 checksum and
    // a non-empty tenaamstelling.
    let iban = validate_iban(&body.iban).map_err(|m| bad_request(&m))?;
    let tenaamstelling = body.tenaamstelling.trim().to_string();
    if tenaamstelling.is_empty() {
        return Err(bad_request("Vul de tenaamstelling van de rekening in."));
    }

    // Facts for art. 27: the (mocked) IBAN-name check and the machtiging.
    let op_naam = naam_komt_overeen(&tenaamstelling, Some(&partij.naam));
    let tekenbevoegd = machtiging::session_machtiging(&session).await == Machtiging::Volledig;
    let vandaag = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let toets = crate::engine::evaluate_rekening(
        state.corpus.clone(),
        op_naam,
        tekenbevoegd,
        true,
        vandaag,
    )
    .await
    .map_err(internal_error)?;
    if !toets.mag_rekening_wijzigen {
        return Err(forbidden_with(
            "Alleen het tekenbevoegd bestuur van de rechtspersoon kan het rekeningnummer \
             opgeven of wijzigen (artikel 27 Wpp). Uw beperkte machtiging als \
             afdelingsbestuurder volstaat hiervoor niet; log in namens de gehele partij.",
        ));
    }
    if !toets.rekening_aanvaardbaar {
        return Err(bad_request(&format!(
            "De tenaamstelling '{tenaamstelling}' komt niet overeen met de geregistreerde \
             aanduiding '{}'. De rekening moet op naam van de rechtspersoon staan \
             (artikel 27 Wpp; IBAN-naam-controle gesimuleerd).",
            partij.naam
        )));
    }
    register::update_rekening(&state.pool, &kvk, &iban, &tenaamstelling)
        .await
        .map_err(internal_error)?;
    tracing::info!(kvk = %kvk, "rekening van de rechtspersoon vastgelegd (IBAN-naam-controle gesimuleerd)");

    // Het feit "rekening bekend" is veranderd; art. 27 heft daarmee de
    // aanhouding op (de toets hierboven gaf al uitbetaling_aangehouden =
    // false voor deze rekening). Aangehouden betaalopdrachten worden
    // alsnog klaargezet voor het betaalsysteem.
    let mut geactiveerd = 0;
    if !toets.uitbetaling_aangehouden {
        for opdracht_id in crate::db::aangehouden_betaalopdrachten(&state.pool, &kvk)
            .await
            .map_err(internal_error)?
        {
            crate::db::activeer_betaalopdracht(&state.pool, &opdracht_id, &iban, &tenaamstelling)
                .await
                .map_err(internal_error)?;
            geactiveerd += 1;
            tracing::info!(opdracht = %opdracht_id, "aangehouden betaalopdracht geactiveerd (art. 27 Wpp)");
        }
    }

    Ok(Json(json!({
        "iban": iban,
        "tenaamstelling": tenaamstelling,
        "in_register": true,
        "geactiveerde_betaalopdrachten": geactiveerd,
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // -- mod-97 ---------------------------------------------------------------

    #[test]
    fn valid_ibans_pass_and_are_normalized() {
        assert_eq!(
            validate_iban("NL91ABNA0417164300").unwrap(),
            "NL91ABNA0417164300"
        );
        // Spaces and lowercase are normalized away.
        assert_eq!(
            validate_iban("nl91 abna 0417 1643 00").unwrap(),
            "NL91ABNA0417164300"
        );
        // Other valid IBANs (different banks / check digits).
        assert!(validate_iban("NL69INGB0123456789").is_ok());
        assert!(validate_iban("NL02ABNA0123456789").is_ok());
    }

    #[test]
    fn invalid_ibans_are_rejected() {
        // One digit changed: mod-97 fails.
        assert!(validate_iban("NL91ABNA0417164301").is_err());
        // Check digits changed.
        assert!(validate_iban("NL92ABNA0417164300").is_err());
        // Two account digits swapped.
        assert!(validate_iban("NL91ABNA0417164030").is_err());
        // Structurally invalid.
        assert!(validate_iban("").is_err());
        assert!(validate_iban("NL91").is_err());
        assert!(validate_iban("91NLABNA0417164300").is_err());
        assert!(validate_iban("NL91ABNA-417164300").is_err());
        assert!(validate_iban("NL91ABNA04171643000000000000000000000").is_err());
    }

    #[test]
    fn iban_error_message_is_dutch() {
        let message = validate_iban("NL92ABNA0417164300").unwrap_err();
        assert!(message.contains("controlecijfers"), "melding: {message}");
    }

    // -- mock name check (fact producer; the verdict is art. 27, engine) ------

    #[test]
    fn matching_tenaamstelling_variants_match() {
        assert!(naam_komt_overeen("D66", Some("D66")));
        assert!(naam_komt_overeen("d66", Some("D66")));
        assert!(naam_komt_overeen("Vereniging D66", Some("D66")));
        // Partial but clearly the same party.
        assert!(naam_komt_overeen(
            "Hart voor Den Haag",
            Some("Hart voor Den Haag / Groep de Mos")
        ));
    }

    #[test]
    fn clear_mismatch_does_not_match() {
        assert!(!naam_komt_overeen("Bakkerij Jansen B.V.", Some("D66")));
        assert!(!naam_komt_overeen("VVD", Some("D66")));
        assert!(!naam_komt_overeen(
            "Penningmeester privé",
            Some("Hart voor Den Haag / Groep de Mos")
        ));
    }

    #[test]
    fn without_registered_name_there_is_nothing_to_compare() {
        // Unknown organization: nothing to compare the name against.
        assert!(naam_komt_overeen("Wat dan ook", None));
    }
}
