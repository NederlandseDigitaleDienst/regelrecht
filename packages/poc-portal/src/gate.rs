//! The password gate.
//!
//! One shared password per PoC, held in an environment variable the platform
//! injects, and a signed cookie so the visitor types it once. No accounts, no
//! database: the thing being protected is a demo, and the people who get in are
//! the people someone sent a link and a password to.
//!
//! What the cookie says is exactly "this browser knew the password for this
//! slug" — no identity, and no rights inside the PoC. Whatever a PoC does with
//! its own login (napp keeps its mock SSO) sits behind this, untouched.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

type HmacSha256 = Hmac<Sha256>;

/// How long a visitor stays in after typing the password.
pub const GELDIGHEID: time::Duration = time::Duration::days(30);

/// The cookie name for a slug. Scoped per PoC, so a browser that may see one
/// PoC does not send anything that would open another.
pub fn cookie_naam(slug: &str) -> String {
    format!("poc_{}", slug.replace('-', "_"))
}

/// The signing key, from `POC_COOKIE_SECRET`.
///
/// Deliberately not derived from the passwords: rotating one password would
/// otherwise invalidate every other PoC's cookies, and anyone holding one
/// password could forge cookies for the slug they already had.
#[derive(Clone)]
pub struct Sleutel(Vec<u8>);

impl Sleutel {
    /// At least 32 bytes. A short secret is the kind of mistake that still
    /// works in testing and quietly makes forgery cheap, so it is refused.
    pub fn new(secret: &str) -> Result<Self, &'static str> {
        if secret.len() < 32 {
            return Err("POC_COOKIE_SECRET must be at least 32 characters");
        }
        Ok(Self(secret.as_bytes().to_vec()))
    }

    fn sign(&self, slug: &str, verloopt: i64) -> String {
        // `expect` over `?`: HMAC accepts a key of any length, so the only way
        // this fails is a broken crate contract.
        #[allow(clippy::expect_used)]
        let mut mac = HmacSha256::new_from_slice(&self.0).expect("HMAC accepts any key length");
        mac.update(bericht(slug, verloopt).as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }
}

/// What gets signed. The slug is inside the message, so a cookie minted for one
/// PoC cannot be replayed against another by renaming it.
fn bericht(slug: &str, verloopt: i64) -> String {
    format!("{slug}.{verloopt}")
}

/// Mint a cookie value: `<expiry>.<signature>`.
pub fn maak_cookie(sleutel: &Sleutel, slug: &str, nu: time::OffsetDateTime) -> String {
    let verloopt = (nu + GELDIGHEID).unix_timestamp();
    format!("{verloopt}.{}", sleutel.sign(slug, verloopt))
}

/// Does this cookie value open this slug, right now?
pub fn cookie_is_geldig(
    sleutel: &Sleutel,
    slug: &str,
    waarde: &str,
    nu: time::OffsetDateTime,
) -> bool {
    let Some((verloopt_raw, handtekening)) = waarde.split_once('.') else {
        return false;
    };
    let Ok(verloopt) = verloopt_raw.parse::<i64>() else {
        return false;
    };

    // Check the signature before the clock. Both orders are safe here, but this
    // one never reports "expired" about a value we have not authenticated.
    let verwacht = sleutel.sign(slug, verloopt);
    let klopt: bool = verwacht.as_bytes().ct_eq(handtekening.as_bytes()).into();
    if !klopt {
        return false;
    }

    nu.unix_timestamp() < verloopt
}

/// Does `ingevoerd` match the PoC's password?
///
/// Compares SHA-256 digests in constant time — the idiom
/// `packages/admin/src/middleware.rs` uses — so neither the password's content
/// nor its length leaks through timing.
pub fn wachtwoord_klopt(verwacht: &str, ingevoerd: &str) -> bool {
    use sha2::Digest;
    let a = Sha256::digest(verwacht.as_bytes());
    let b = Sha256::digest(ingevoerd.as_bytes());
    a.ct_eq(&b).into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn sleutel() -> Sleutel {
        Sleutel::new(SECRET).expect("32 chars")
    }

    fn nu() -> time::OffsetDateTime {
        time::OffsetDateTime::UNIX_EPOCH + time::Duration::days(20_000)
    }

    #[test]
    fn a_short_secret_is_refused() {
        assert!(Sleutel::new("te-kort").is_err());
        assert!(Sleutel::new(SECRET).is_ok());
    }

    #[test]
    fn a_fresh_cookie_opens_its_own_slug() {
        let s = sleutel();
        let c = maak_cookie(&s, "napp", nu());
        assert!(cookie_is_geldig(&s, "napp", &c, nu()));
    }

    #[test]
    fn a_cookie_does_not_open_another_slug() {
        // The whole point of per-PoC passwords: holding one must not hand over
        // the next. The slug is signed, so renaming the cookie does not work.
        let s = sleutel();
        let c = maak_cookie(&s, "napp", nu());
        assert!(!cookie_is_geldig(&s, "terugbetaalregimes", &c, nu()));
    }

    #[test]
    fn a_cookie_expires() {
        let s = sleutel();
        let c = maak_cookie(&s, "napp", nu());
        let later = nu() + GELDIGHEID + time::Duration::seconds(1);
        assert!(!cookie_is_geldig(&s, "napp", &c, later));
    }

    #[test]
    fn another_secret_does_not_open_it() {
        let c = maak_cookie(&sleutel(), "napp", nu());
        let ander = Sleutel::new("ffffffffffffffffffffffffffffffff").expect("32 chars");
        assert!(!cookie_is_geldig(&ander, "napp", &c, nu()));
    }

    #[test]
    fn a_visitor_cannot_extend_their_own_cookie() {
        // The expiry is inside the signed message, so editing it invalidates
        // the signature rather than buying more time.
        let s = sleutel();
        let c = maak_cookie(&s, "napp", nu());
        let (_, sig) = c.split_once('.').expect("shape");
        let ver = (nu() + time::Duration::days(3650)).unix_timestamp();
        assert!(!cookie_is_geldig(&s, "napp", &format!("{ver}.{sig}"), nu()));
    }

    #[test]
    fn malformed_cookies_are_refused_rather_than_panicking() {
        let s = sleutel();
        for bad in ["", ".", "abc", "abc.def", "...", "9999999999."] {
            assert!(!cookie_is_geldig(&s, "napp", bad, nu()), "{bad:?}");
        }
    }

    #[test]
    fn the_password_must_match_exactly() {
        assert!(wachtwoord_klopt("hunter2", "hunter2"));
        assert!(!wachtwoord_klopt("hunter2", "hunter"));
        assert!(!wachtwoord_klopt("hunter2", "hunter22"));
        assert!(!wachtwoord_klopt("hunter2", ""));
    }

    #[test]
    fn the_cookie_name_is_a_valid_cookie_name() {
        // A hyphen is legal in a cookie name, but the env var it pairs with
        // uses an underscore; keeping both on underscores avoids two spellings
        // of the same PoC.
        assert_eq!(cookie_naam("mijn-poc"), "poc_mijn_poc");
    }
}
