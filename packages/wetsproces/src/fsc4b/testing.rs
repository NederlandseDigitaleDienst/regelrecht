//! Test support: a Stelsel whose keys a test holds, so it can mint any
//! credential, good or bad, and an app that answers challenges. For the
//! unit tests and, with the feature `test-utils`, the end-to-end tests.
//! Every key is made fresh per test; nothing here is a fixed key. Every
//! function panics when the library fails, as a test should.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Map, Value, json};

use crate::config::{Fsc4bSettings, KeySource};
use crate::fsc4b::demo::{credential, stelseldocument};
use crate::fsc4b::jose::{self, AesKey, Jws, SigningKey};
use crate::fsc4b::presentation::{APP_MANAGER_CERTIFICATE, CHALLENGE_RESPONSE};
use crate::fsc4b::session::Complete;
use crate::fsc4b::store::InMemoryFsc4bStore;
use crate::fsc4b::trust::{Trust, TrustInput};
use crate::fsc4b::{DISCOVERY, Fsc4b};

/// The made-up OINs and scope of a test Stelsel.
pub const OWN_OIN: &str = "00000099000000000011";
pub const ISSUER_OIN: &str = "00000099000000000012";
pub const SCOPE: &str = "nl.voorbeeld.wetsproces.test";

/// A Stelsel whose every key the test holds.
pub struct TestStelsel {
    pub beheerder: SigningKey,
    pub issuer: SigningKey,
    pub own: SigningKey,
    pub document: String,
}

impl Default for TestStelsel {
    fn default() -> Self {
        Self::new()
    }
}

impl TestStelsel {
    pub fn new() -> Self {
        Self::at("http://localhost")
    }

    /// A Stelsel that lists the organization's discovery configuration on
    /// the server at `public_url`.
    pub fn at(public_url: &str) -> Self {
        let beheerder = SigningKey::generate().unwrap();
        let issuer = SigningKey::generate().unwrap();
        let own = SigningKey::generate().unwrap();
        let document = stelseldocument(
            &beheerder,
            (
                OWN_OIN,
                own.public_jwk(),
                &format!("{public_url}{DISCOVERY}"),
            ),
            (ISSUER_OIN, issuer.public_jwk()),
        )
        .unwrap();
        Self {
            beheerder,
            issuer,
            own,
            document,
        }
    }

    pub fn own_oin(&self) -> &str {
        OWN_OIN
    }

    pub fn trust(&self) -> Trust {
        let own = SigningKey::from_pkcs8_pem(&self.own.to_pkcs8_pem().unwrap()).unwrap();
        Trust::build(TrustInput {
            own_oin: OWN_OIN,
            key: own,
            scope: SCOPE,
            stelseldocument: &self.document,
            stelselbeheerder: &self.beheerder.verifying_key().unwrap(),
        })
        .unwrap()
    }

    /// The channel on this Stelsel, in memory.
    pub fn channel(&self) -> Fsc4b {
        Fsc4b::new(
            self.trust(),
            Arc::new(InMemoryFsc4bStore::default()),
            "http://localhost".into(),
            None,
        )
    }

    /// The configuration of this Stelsel for a server at `public_url`,
    /// with its files written into `dir`.
    pub fn settings(&self, dir: &Path, public_url: &str) -> Fsc4bSettings {
        let document = dir.join("stelseldocument.jws");
        std::fs::write(&document, &self.document).unwrap();
        let key = dir.join("organisatie.pem");
        std::fs::write(&key, self.own.to_pkcs8_pem().unwrap()).unwrap();
        Fsc4bSettings {
            oin: OWN_OIN.into(),
            scope: SCOPE.into(),
            key: KeySource::File(key),
            stelseldocument: document,
            stelselbeheerder_key: serde_json::to_string(self.beheerder.public_jwk()).unwrap(),
            public_url: public_url.into(),
        }
    }

    /// A good credential for `app` and the BSN `bsn`, from `now` for an
    /// hour, after `adjust` changed its claims as a test wants.
    pub fn credential_for(
        &self,
        app: &TestApp,
        bsn: &str,
        now: i64,
        adjust: impl FnOnce(&mut Map<String, Value>),
    ) -> String {
        self.credential_by(&self.issuer, app, bsn, now, adjust)
    }

    /// [`Self::credential_for`] with a fixed test BSN.
    pub fn credential(
        &self,
        app: &TestApp,
        now: i64,
        adjust: impl FnOnce(&mut Map<String, Value>),
    ) -> String {
        self.credential_for(app, "999990019", now, adjust)
    }

    /// A credential signed by `issuer`, which may be no trusted one.
    pub fn credential_by(
        &self,
        issuer: &SigningKey,
        app: &TestApp,
        bsn: &str,
        now: i64,
        adjust: impl FnOnce(&mut Map<String, Value>),
    ) -> String {
        let good = credential(
            issuer,
            ISSUER_OIN,
            SCOPE,
            app.key.public_jwk(),
            (bsn, "Test", "Burger"),
            now,
            now + 3600,
        )
        .unwrap();
        let mut claims = Jws::unverified(&good).unwrap().claims;
        adjust(&mut claims);
        issuer.sign(&Value::Object(claims), &[]).unwrap()
    }
}

/// An app: its key pair and the session key it picks.
pub struct TestApp {
    pub key: SigningKey,
    session_key: AesKey,
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TestApp {
    pub fn new() -> Self {
        Self {
            key: SigningKey::generate().unwrap(),
            session_key: AesKey::generate().unwrap(),
        }
    }

    /// An app with the key pair `key`.
    pub fn with_key(key: SigningKey) -> Self {
        Self {
            key,
            session_key: AesKey::generate().unwrap(),
        }
    }

    pub fn session_key(&self) -> &AesKey {
        &self.session_key
    }

    /// The signature over `nonce` for the verifier `aud`.
    pub fn sign_nonce(&self, nonce: &str, aud: &str) -> String {
        self.key
            .sign(
                &json!({ "sub": CHALLENGE_RESPONSE, "aud": aud, "nonce": nonce }),
                &[],
            )
            .unwrap()
    }

    /// The presentation of `credential` for the challenge `nonce`, before
    /// encryption, as JSON.
    pub fn presentation(&self, nonce: &str, credential: &str, aud: &str) -> Value {
        json!({
            "app_nonce_signature": self.sign_nonce(nonce, aud),
            "certificate_type": APP_MANAGER_CERTIFICATE,
            "certificate": credential,
            "session_aes_key": jose::b64(self.session_key.as_bytes()),
        })
    }

    /// The answer to the challenge `jws` of `fsc`, as `/v1/complete` takes
    /// it.
    pub fn answer(&self, fsc: &Fsc4b, jws: &str, credential: &str, aud: &str) -> Complete {
        let challenge = fsc
            .trust()
            .key()
            .verifying_key()
            .unwrap()
            .verify(jws)
            .unwrap();
        let nonce = challenge.text("nonce").unwrap().to_string();
        let der = jose::unb64(
            challenge
                .text("ephemeral_organization_ec_public_key")
                .unwrap(),
            "der",
        )
        .unwrap();
        let vp = self.presentation(&nonce, credential, aud);
        Complete {
            nonce,
            encrypted_challenge_result: jose::encrypt_to_der(&der, vp.to_string().as_bytes())
                .unwrap(),
        }
    }
}

/// `jwe` with one bit of its ciphertext flipped.
pub fn flip_ciphertext_bit(jwe: &str) -> String {
    let mut segments: Vec<String> = jwe.split('.').map(str::to_string).collect();
    let mut ciphertext = URL_SAFE_NO_PAD.decode(&segments[3]).unwrap();
    ciphertext[0] ^= 1;
    segments[3] = URL_SAFE_NO_PAD.encode(ciphertext);
    segments.join(".")
}

/// A decider of the municipality decides on the submission `op` of the
/// server at `base`, through the test login of the browser channel; the id
/// of the decision comes back. For a test that asks why a decision came
/// out so.
pub async fn decide_as_municipality(http: &reqwest::Client, base: &str, op: &str) -> String {
    let login = http
        .post(format!("{base}/auth/testlogin"))
        .json(&json!({
            "sub": "gemeente-1",
            "rollen": ["wetsproces-beslisser", "wetsproces-cel-gemeente_wilgendaal"],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), reqwest::StatusCode::OK);
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let findings = json!({
        "langdurige_loopbeperking_volgens_college": true,
        "continu_afhankelijk_van_hulp_bestuurder_volgens_college": false,
        "permanent_rolstoelgebonden_volgens_college": false,
        "ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college": false,
        "geneeskundig_onderzoek_achterwege_gelaten": false,
        "bekend_dat_aanvrager_nog_steeds_voldoet": false,
        "keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet": false,
    });
    let besluit: Value = http
        .post(format!("{base}/api/backoffice/besluiten"))
        .header("cookie", cookie)
        .json(&json!({ "op": op, "bevindingen": findings }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    besluit["besluit"]["id"].as_str().unwrap().to_string()
}
