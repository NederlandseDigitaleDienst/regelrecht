//! What identifies the person who applies: the claim of their account that
//! the login fills ([`crate::api::AppState::subject_claim`]), and how the
//! simulated login of test mode checks a number typed into it.
//!
//! A citizen logs in with a citizen service number (BSN, claim `bsn`), as
//! with DigiD; a business with its Chamber of Commerce number (claim
//! `kvk_nummer`), as with eHerkenning. One login handles both, with the
//! label and the check of [`Identifier`]: a BSN of the test range passes
//! the eleven test, a KvK number is eight digits. Another claim takes any
//! short text.

use regelrecht_fsc4b::wire::is_test_bsn;
use serde_json::{Value, json};

/// The claim of a citizen service number.
pub const BSN: &str = "bsn";

/// The claim of a Chamber of Commerce number, as the case's login names
/// it (`inloggen.sleutel`).
pub const KVK: &str = "kvk_nummer";

/// The longest identifier of another claim the simulated login takes, in
/// characters.
const MAX_OTHER_CHARS: usize = 64;

/// What kind of number identifies an applicant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A citizen service number of the test range.
    Bsn,
    /// A Chamber of Commerce number: eight digits.
    Kvk,
    /// Another claim, without a check of its form.
    Other,
}

/// The claim that identifies an applicant, and how its value is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    claim: String,
    kind: Kind,
}

impl Identifier {
    /// The identifier of the claim `claim`.
    pub fn for_claim(claim: &str) -> Self {
        let kind = match claim {
            BSN => Kind::Bsn,
            KVK | "kvk" => Kind::Kvk,
            _ => Kind::Other,
        };
        Self {
            claim: claim.to_owned(),
            kind,
        }
    }

    /// The claim, as the session keeps it.
    pub fn claim(&self) -> &str {
        &self.claim
    }

    /// The kind of number.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// What a screen calls the number.
    pub fn label(&self) -> &str {
        match self.kind {
            Kind::Bsn => "BSN",
            Kind::Kvk => "KvK-nummer",
            Kind::Other => &self.claim,
        }
    }

    /// The `sub` of the account of the simulated login with `value`: a
    /// citizen as before, a business apart, so the two never share one.
    pub fn subject(&self, value: &str) -> String {
        match self.kind {
            Kind::Bsn | Kind::Other => format!("burger-{value}"),
            Kind::Kvk => format!("bedrijf-{value}"),
        }
    }

    /// What the simulated login screen needs to know of the number: the
    /// claim, its label, its kind (`bsn`, `kvk` or `anders`), and how many
    /// digits it has when that is fixed.
    pub fn describe(&self) -> Value {
        let (kind, digits) = match self.kind {
            Kind::Bsn => ("bsn", Some(9)),
            Kind::Kvk => ("kvk", Some(8)),
            Kind::Other => ("anders", None),
        };
        json!({
            "claim": self.claim,
            "label": self.label(),
            "soort": kind,
            "cijfers": digits,
        })
    }

    /// The number in `text`, with spaces around it left out.
    ///
    /// # Errors
    ///
    /// A message for the applicant: a BSN that is not nine digits, fails the
    /// eleven test or lies outside the test range; a KvK number that is not
    /// eight digits; another identifier that is empty, too long or holds a
    /// space or a control character.
    pub fn parse(&self, text: &str) -> Result<String, &'static str> {
        let text = text.trim();
        let digits = |n: usize| text.len() == n && text.bytes().all(|b| b.is_ascii_digit());
        match self.kind {
            Kind::Bsn => {
                if !digits(9) {
                    return Err("Een BSN bestaat uit negen cijfers.");
                }
                if !passes_eleven_test(text) {
                    return Err("Dit BSN haalt de elfproef niet. Kijk de cijfers na.");
                }
                if !is_test_bsn(text) {
                    return Err(
                        "Deze simulatie neemt alleen een BSN uit de testreeks aan: negen cijfers die beginnen met 99999.",
                    );
                }
            }
            Kind::Kvk => {
                if !digits(8) {
                    return Err("Een KvK-nummer bestaat uit acht cijfers.");
                }
            }
            Kind::Other => {
                if text.is_empty()
                    || text.chars().count() > MAX_OTHER_CHARS
                    || text.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err("Vul een nummer in, zonder spaties.");
                }
            }
        }
        Ok(text.to_owned())
    }
}

/// The eleven test of a citizen service number of nine ASCII digits: the
/// first eight times 9 down to 2, minus the last, is a multiple of 11.
fn passes_eleven_test(digits: &str) -> bool {
    let sum: i64 = digits
        .bytes()
        .zip([9, 8, 7, 6, 5, 4, 3, 2, -1])
        .map(|(b, weight)| i64::from(b - b'0') * weight)
        .sum();
    sum % 11 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use regelrecht_fsc4b::wire::TEST_CITIZENS;

    #[test]
    fn a_bsn_is_nine_digits_that_pass_the_eleven_test_in_the_test_range() {
        let bsn = Identifier::for_claim(BSN);
        for citizen in TEST_CITIZENS {
            assert_eq!(bsn.parse(citizen), Ok(citizen.to_owned()));
        }
        assert_eq!(bsn.parse(" 999990032 "), Ok("999990032".to_owned()));
        assert_eq!(
            bsn.parse("12345"),
            Err("Een BSN bestaat uit negen cijfers.")
        );
        assert_eq!(
            bsn.parse("999990033"),
            Err("Dit BSN haalt de elfproef niet. Kijk de cijfers na.")
        );
        // Passes the eleven test, outside the test range.
        assert!(bsn.parse("111222333").is_err());
    }

    #[test]
    fn a_kvk_number_is_eight_digits() {
        let kvk = Identifier::for_claim(KVK);
        assert_eq!(kvk.kind(), Kind::Kvk);
        assert_eq!(kvk.parse("90123456"), Ok("90123456".to_owned()));
        for bad in ["9012345", "901234567", "9012345a", ""] {
            assert_eq!(
                kvk.parse(bad),
                Err("Een KvK-nummer bestaat uit acht cijfers.")
            );
        }
        assert_eq!(kvk.subject("90123456"), "bedrijf-90123456");
        assert_eq!(kvk.describe()["label"], "KvK-nummer");
        assert_eq!(kvk.describe()["cijfers"], 8);
    }

    #[test]
    fn another_claim_takes_a_short_text_without_spaces() {
        let other = Identifier::for_claim("rsin");
        assert_eq!(other.kind(), Kind::Other);
        assert_eq!(other.label(), "rsin");
        assert!(other.parse("123").is_ok());
        assert!(other.parse("1 2").is_err());
        assert!(other.parse("").is_err());
    }
}
