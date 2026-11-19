//! The checks of a Verifiable Presentation (§4.3.4.1) and the Verifiable
//! Credential in it (§4.3.1.1), in the order the specification gives them.
//! Each failed check is its own [`Refusal`], for the log and the tests; the
//! app is told only that its presentation was not accepted.

use serde_json::{Map, Value};
use subtle::ConstantTimeEq;

use crate::fsc4b::jose::{self, AES_KEY_LEN, AesKey, JoseError, Jws, VerifyingKey};
use crate::fsc4b::trust::Trust;

/// The leeway on `nbf` and `exp`, for clocks that differ a little (RFC 7519
/// §4.1.4, §4.1.5 allow "some small leeway").
pub const CLOCK_LEEWAY_SECS: i64 = 30;

/// The `certificate_type` of a credential from an Activatiedienst.
pub const APP_MANAGER_CERTIFICATE: &str = "app_manager_jwt_certificate";

/// The `certificate_type` of a self-signed key (§4.3.4.1).
pub const SELF_SIGNED_CERTIFICATE: &str = "certificate_type_self_signed";

/// The fixed `sub` of the app's signature over the nonce.
pub const CHALLENGE_RESPONSE: &str = "challenge_response";

/// Why a presentation is not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the presentation is no JSON object with the fields of §4.3.4.1")]
    Shape,
    #[error(
        "the session_aes_key is not {AES_KEY_LEN} bytes; see the docs on the choice of key length"
    )]
    SessionKeyLength,
    #[error("a self-signed certificate carries no identity, and this service needs one")]
    SelfSigned,
    #[error("the certificate_type `{0}` is unknown")]
    CertificateType(String),
    #[error("the credential: {0}")]
    CredentialToken(JoseError),
    #[error("the credential names no trusted Activatiedienst")]
    UnknownIssuer,
    #[error("the credential's signature: {0}")]
    CredentialSignature(JoseError),
    #[error("the credential lacks `{0}`, or holds it with the wrong type")]
    CredentialField(&'static str),
    #[error("the credential is not valid yet")]
    NotYetValid,
    #[error("the credential has expired")]
    Expired,
    #[error("the credential is for another scope")]
    Scope,
    #[error("the credential's BSN is no nine digits")]
    Bsn,
    #[error("the app key in `cnf`: {0}")]
    AppKey(JoseError),
    #[error("the signature over the nonce: {0}")]
    NonceSignature(JoseError),
    #[error("the signature over the nonce has the wrong `sub`")]
    NonceSub,
    #[error("the signature over the nonce is for another verifier (`aud`)")]
    Audience,
    #[error("the signature over the nonce is over another nonce")]
    Nonce,
}

/// A credential that passed every check: who the citizen is and the key of
/// their app.
#[derive(Debug)]
pub struct VerifiedCredential {
    pub bsn: String,
    pub given_name: String,
    pub family_name: String,
    pub scope: String,
    pub exp: i64,
    pub app_key: VerifyingKey,
}

/// A presentation that passed every check.
#[derive(Debug)]
pub struct Verified {
    pub credential: VerifiedCredential,
    pub session_key: AesKey,
}

fn text<'a>(claims: &'a Map<String, Value>, name: &'static str) -> Result<&'a str, Refusal> {
    claims
        .get(name)
        .and_then(Value::as_str)
        .ok_or(Refusal::CredentialField(name))
}

fn number(claims: &Map<String, Value>, name: &'static str) -> Result<i64, Refusal> {
    claims
        .get(name)
        .and_then(Value::as_i64)
        .ok_or(Refusal::CredentialField(name))
}

/// The checks of §4.3.1.1 on the credential `jws`, at the Unix time `now`.
///
/// # Errors
///
/// The [`Refusal`] of the first check that fails.
pub fn verify_credential(
    trust: &Trust,
    jws: &str,
    now: i64,
) -> Result<VerifiedCredential, Refusal> {
    // 1. Find the Activatiedienst's key by `iss` (and `kid`), from the
    //    unverified token.
    let unverified = Jws::unverified(jws).map_err(Refusal::CredentialToken)?;
    let iss = unverified
        .text("iss")
        .ok_or(Refusal::CredentialField("iss"))?;
    // 2. Verify the signature under one of its keys, and `nbf` and `exp`.
    let mut refusal = Refusal::UnknownIssuer;
    let mut verified = None;
    for issuer in trust.issuers(iss, unverified.header_text("kid")) {
        match issuer.key().verify(jws) {
            Ok(jws) => {
                verified = Some(jws);
                break;
            }
            Err(e) => refusal = Refusal::CredentialSignature(e),
        }
    }
    let claims = verified.ok_or(refusal)?.claims;
    let nbf = number(&claims, "nbf")?;
    let exp = number(&claims, "exp")?;
    if now.saturating_add(CLOCK_LEEWAY_SECS) < nbf {
        return Err(Refusal::NotYetValid);
    }
    if now.saturating_sub(CLOCK_LEEWAY_SECS) >= exp {
        return Err(Refusal::Expired);
    }
    // 3. Every field, of the right type.
    number(&claims, "iat")?;
    let bsn = text(&claims, "bsn")?;
    let given_name = text(&claims, "given_name")?;
    let family_name = text(&claims, "family_name")?;
    let scope = text(&claims, "scope")?;
    let jwk = claims
        .get("cnf")
        .and_then(|c| c.get("jwk"))
        .ok_or(Refusal::CredentialField("cnf"))?;
    // 4. The scope this service expects.
    if scope != trust.scope() {
        return Err(Refusal::Scope);
    }
    if bsn.len() != 9 || !bsn.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Refusal::Bsn);
    }
    // 5. The app's key: a supported curve, on the curve.
    let app_key = VerifyingKey::from_value(jwk).map_err(Refusal::AppKey)?;
    Ok(VerifiedCredential {
        bsn: bsn.to_string(),
        given_name: given_name.to_string(),
        family_name: family_name.to_string(),
        scope: scope.to_string(),
        exp,
        app_key,
    })
}

/// The checks of §4.3.4.1 on the presentation `vp` (JSON), answering the
/// challenge with `nonce`, at the Unix time `now`.
///
/// # Errors
///
/// The [`Refusal`] of the first check that fails.
pub fn verify_presentation(
    trust: &Trust,
    vp: &[u8],
    nonce: &str,
    now: i64,
) -> Result<Verified, Refusal> {
    // 1. A JSON object with the four fields.
    let Ok(Value::Object(vp)) = serde_json::from_slice::<Value>(vp) else {
        return Err(Refusal::Shape);
    };
    let field = |name: &str| vp.get(name).and_then(Value::as_str).ok_or(Refusal::Shape);
    let signature = field("app_nonce_signature")?;
    let certificate_type = field("certificate_type")?;
    let certificate = field("certificate")?;
    let session_key = field("session_aes_key")?;
    // 2. The session key, of the length this server holds to (see
    //    `AES_KEY_LEN`).
    let session_key = jose::unb64(session_key, "the session key")
        .ok()
        .and_then(|b| AesKey::from_slice(&b).ok())
        .ok_or(Refusal::SessionKeyLength)?;
    // 3. The certificate is a JWS.
    Jws::unverified(certificate).map_err(Refusal::CredentialToken)?;
    // 4. Its type: only a credential identifies a citizen.
    let credential = match certificate_type {
        APP_MANAGER_CERTIFICATE => verify_credential(trust, certificate, now)?,
        SELF_SIGNED_CERTIFICATE => return Err(Refusal::SelfSigned),
        other => return Err(Refusal::CertificateType(other.to_string())),
    };
    // 5. The nonce, signed by the key in the credential, for this verifier.
    let signed = credential
        .app_key
        .verify(signature)
        .map_err(Refusal::NonceSignature)?;
    if signed.text("sub") != Some(CHALLENGE_RESPONSE) {
        return Err(Refusal::NonceSub);
    }
    if signed.text("aud") != Some(trust.own_oin().as_str()) {
        return Err(Refusal::Audience);
    }
    let signed_nonce = signed.text("nonce").unwrap_or_default();
    if !bool::from(signed_nonce.as_bytes().ct_eq(nonce.as_bytes())) {
        return Err(Refusal::Nonce);
    }
    Ok(Verified {
        credential,
        session_key,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::fsc4b::jose::SigningKey;
    use crate::fsc4b::testing::{ISSUER_OIN, OWN_OIN, TestApp, TestStelsel};

    const NOW: i64 = 1_800_000_000;
    const NONCE: &str = "BZpwbYf2TIYxvJEQY2FtAg==";

    /// The presentation of `credential` by `app` for [`NONCE`] and `aud`.
    fn vp(app: &TestApp, credential: &str, aud: &str) -> Vec<u8> {
        app.presentation(NONCE, credential, aud)
            .to_string()
            .into_bytes()
    }

    fn refusal(stelsel: &TestStelsel, vp: &[u8]) -> Refusal {
        verify_presentation(&stelsel.trust(), vp, NONCE, NOW).unwrap_err()
    }

    #[test]
    fn a_good_presentation_gives_the_bsn_and_the_session_key() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |_| {});
        let v = verify_presentation(&s.trust(), &vp(&app, &c, OWN_OIN), NONCE, NOW).unwrap();
        assert_eq!(v.credential.bsn, "999990019");
        assert_eq!(v.session_key.as_bytes(), app.session_key().as_bytes());
        assert_eq!(v.credential.app_key.jwk(), app.key.public_jwk());
    }

    #[test]
    fn an_expired_credential_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW - 7200, |_| {});
        assert_eq!(refusal(&s, &vp(&app, &c, OWN_OIN)), Refusal::Expired);
    }

    #[test]
    fn a_credential_not_valid_yet_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW + 600, |_| {});
        assert_eq!(refusal(&s, &vp(&app, &c, OWN_OIN)), Refusal::NotYetValid);
    }

    #[test]
    fn a_credential_for_another_scope_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |c| {
            c.insert("scope".into(), json!("nl.vorijk.oauth_scope.blauwe_knop"));
        });
        assert_eq!(refusal(&s, &vp(&app, &c, OWN_OIN)), Refusal::Scope);
    }

    #[test]
    fn a_credential_of_an_unknown_activatiedienst_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |c| {
            c.insert("iss".into(), json!("00000099000000000099"));
        });
        assert_eq!(refusal(&s, &vp(&app, &c, OWN_OIN)), Refusal::UnknownIssuer);
    }

    #[test]
    fn a_credential_signed_by_an_untrusted_key_in_a_trusted_name_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let impostor = SigningKey::generate().unwrap();
        let c = s.credential_by(&impostor, &app, "999990019", NOW, |_| {});
        assert!(matches!(
            refusal(&s, &vp(&app, &c, OWN_OIN)),
            Refusal::CredentialSignature(JoseError::Signature)
        ));
        // The issuer's OIN was the trusted one all along.
        assert!(c.contains('.') && s.trust().issuer(ISSUER_OIN, None).is_some());
    }

    #[test]
    fn a_credential_under_the_second_key_of_an_activatiedienst_is_accepted() {
        use crate::fsc4b::trust::{Trust, TrustInput};
        let s = TestStelsel::new();
        let rolled = SigningKey::generate().unwrap();
        let doc = s
            .beheerder
            .sign(
                &json!({
                    "version": 2,
                    "organizations": [{ "oin": OWN_OIN, "public_key": s.own.public_jwk() }],
                    "app_managers": [
                        { "oin": ISSUER_OIN, "public_key": s.issuer.public_jwk() },
                        { "oin": ISSUER_OIN, "public_key": rolled.public_jwk() },
                    ],
                }),
                &[],
            )
            .unwrap();
        let trust = Trust::build(TrustInput {
            own_oin: OWN_OIN,
            key: SigningKey::from_pkcs8_pem(&s.own.to_pkcs8_pem().unwrap()).unwrap(),
            scope: crate::fsc4b::testing::SCOPE,
            stelseldocument: &doc,
            stelselbeheerder: &s.beheerder.verifying_key().unwrap(),
        })
        .unwrap();
        let app = TestApp::new();
        let c = s.credential_by(&rolled, &app, "999990019", NOW, |_| {});
        assert!(verify_presentation(&trust, &vp(&app, &c, OWN_OIN), NONCE, NOW).is_ok());
    }

    #[test]
    fn a_credential_bound_to_another_app_key_is_refused() {
        let s = TestStelsel::new();
        let owner = TestApp::new();
        let thief = TestApp::new();
        let stolen = s.credential(&owner, NOW, |_| {});
        assert!(matches!(
            refusal(&s, &vp(&thief, &stolen, OWN_OIN)),
            Refusal::NonceSignature(JoseError::Signature)
        ));
    }

    #[test]
    fn a_nonce_signed_for_another_verifier_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |_| {});
        assert_eq!(
            refusal(&s, &vp(&app, &c, "00000099000000000077")),
            Refusal::Audience
        );
    }

    #[test]
    fn a_signature_over_another_nonce_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |_| {});
        let mut v = app.presentation(NONCE, &c, OWN_OIN);
        v["app_nonce_signature"] = json!(app.sign_nonce("AAAAAAAAAAAAAAAAAAAAAA==", OWN_OIN));
        assert_eq!(refusal(&s, v.to_string().as_bytes()), Refusal::Nonce);
    }

    #[test]
    fn a_self_signed_certificate_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |_| {});
        let mut v = app.presentation(NONCE, &c, OWN_OIN);
        v["certificate_type"] = json!(SELF_SIGNED_CERTIFICATE);
        v["certificate"] = json!(crate::fsc4b::demo::self_signed_certificate(&app.key).unwrap());
        assert_eq!(refusal(&s, v.to_string().as_bytes()), Refusal::SelfSigned);
    }

    #[test]
    fn a_session_key_of_sixteen_bytes_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |_| {});
        let mut v = app.presentation(NONCE, &c, OWN_OIN);
        v["session_aes_key"] = json!("/YyG+FNLinwQqZFgGUribQ==");
        assert_eq!(
            refusal(&s, v.to_string().as_bytes()),
            Refusal::SessionKeyLength
        );
    }

    #[test]
    fn a_credential_without_a_name_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential(&app, NOW, |c| {
            c.remove("family_name");
        });
        assert_eq!(
            refusal(&s, &vp(&app, &c, OWN_OIN)),
            Refusal::CredentialField("family_name")
        );
    }

    #[test]
    fn a_bsn_that_is_no_nine_digits_is_refused() {
        let s = TestStelsel::new();
        let app = TestApp::new();
        let c = s.credential_for(&app, "99999001x", NOW, |_| {});
        assert_eq!(refusal(&s, &vp(&app, &c, OWN_OIN)), Refusal::Bsn);
    }
}
