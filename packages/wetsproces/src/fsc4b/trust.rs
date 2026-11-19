//! Whom the channel trusts, from a Stelseldocument whose signature it
//! checks (§2.2.3, §4.1): the Activatiediensten under `app_managers` issue
//! the credentials it accepts, and the entry of this organization under
//! `organizations` must hold the public key of the key it signs with.
//!
//! The specification leaves the structure of a Stelseldocument to the
//! Stelselbeheerder (§4.1.1.1). This server reads the one §4.1.1.1 gives as
//! an example, signed as a JWS with ES256 (§3.1), with keys as JWKs:
//!
//! ```json
//! {
//!   "version": 1,
//!   "organizations": [
//!     { "oin": "…", "name": "…", "discovery_url": "https://…", "public_key": { "kty": "EC", … } }
//!   ],
//!   "app_managers": [
//!     { "oin": "…", "name": "…", "discovery_url": "https://…", "public_key": { "kty": "EC", … } }
//!   ],
//!   "document_types": []
//! }
//! ```

use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::fsc4b::jose::{JoseError, Jws, SigningKey, VerifyingKey};

/// An organisatie-identificatienummer: 20 digits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Oin(String);

impl Oin {
    /// The number of digits of an OIN.
    pub const LEN: usize = 20;

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Oin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for Oin {
    type Err = TrustError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == Self::LEN && s.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self(s.to_string()))
        } else {
            Err(TrustError::BadOin(s.to_string()))
        }
    }
}

/// Why the channel does not open on a Stelseldocument.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    #[error("`{0}` is no OIN: an OIN has 20 digits")]
    BadOin(String),
    #[error("the Stelselbeheerder's key: {0}")]
    StelselbeheerderKey(JoseError),
    #[error("the Stelseldocument does not verify under the Stelselbeheerder's key: {0}")]
    Signature(JoseError),
    #[error("the Stelseldocument does not read: {0}")]
    Malformed(String),
    #[error("the key of the Activatiedienst {oin}: {source}")]
    IssuerKey { oin: String, source: JoseError },
    #[error("the Stelseldocument names no Activatiedienst")]
    NoIssuers,
    #[error("the Stelseldocument does not list this organization ({0}) under `organizations`")]
    NotListed(Oin),
    #[error(
        "the Stelseldocument lists another public key for this organization ({0}) than the one \
         its private key belongs to"
    )]
    OtherKey(Oin),
    #[error("the organization's private key: {0}")]
    OwnKey(JoseError),
    #[error("the scope is empty")]
    NoScope,
}

#[derive(Debug, Deserialize)]
struct Document {
    version: u64,
    #[serde(default)]
    organizations: Vec<Participant>,
    #[serde(default)]
    app_managers: Vec<Participant>,
}

#[derive(Debug, Deserialize)]
struct Participant {
    oin: String,
    #[serde(default)]
    name: String,
    public_key: Value,
}

/// An Activatiedienst the channel trusts: its OIN and its key.
#[derive(Debug)]
pub struct Issuer {
    oin: Oin,
    name: String,
    key: VerifyingKey,
}

impl Issuer {
    pub fn oin(&self) -> &Oin {
        &self.oin
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn key(&self) -> &VerifyingKey {
        &self.key
    }
}

/// What the channel trusts and who it is: this organization's OIN and key,
/// the scope it accepts, and the Activatiediensten of the Stelseldocument.
pub struct Trust {
    own_oin: Oin,
    key: SigningKey,
    scope: String,
    issuers: Vec<Issuer>,
    stelsel_version: u64,
    demo: bool,
}

impl fmt::Debug for Trust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Trust")
            .field("own_oin", &self.own_oin)
            .field("scope", &self.scope)
            .field("issuers", &self.issuers)
            .field("stelsel_version", &self.stelsel_version)
            .field("demo", &self.demo)
            .finish_non_exhaustive()
    }
}

/// The pieces a [`Trust`] is built from.
pub struct TrustInput<'a> {
    pub own_oin: &'a str,
    pub key: SigningKey,
    pub scope: &'a str,
    /// The Stelseldocument, a compact JWS.
    pub stelseldocument: &'a str,
    pub stelselbeheerder: &'a VerifyingKey,
}

impl Trust {
    /// The trust of `input`: the Stelseldocument verified under the
    /// Stelselbeheerder's key (§2.2.3: "Organisaties … MOETEN de
    /// handtekening van de stelselbeheerder controleren"), its
    /// Activatiediensten, and this organization's entry checked against its
    /// own key.
    ///
    /// # Errors
    ///
    /// When the OIN or scope is malformed, the document does not verify or
    /// read, names no Activatiedienst, or does not list this organization
    /// with the public key of `input.key`.
    pub fn build(input: TrustInput<'_>) -> Result<Self, TrustError> {
        let own_oin: Oin = input.own_oin.parse()?;
        if input.scope.trim().is_empty() {
            return Err(TrustError::NoScope);
        }
        let jws: Jws = input
            .stelselbeheerder
            .verify(input.stelseldocument)
            .map_err(TrustError::Signature)?;
        let document: Document = serde_json::from_value(Value::Object(jws.claims))
            .map_err(|e| TrustError::Malformed(e.to_string()))?;
        let own_key = input.key.verifying_key().map_err(TrustError::OwnKey)?;
        let listed = document
            .organizations
            .iter()
            .find(|o| o.oin == own_oin.as_str())
            .ok_or_else(|| TrustError::NotListed(own_oin.clone()))?;
        let listed_key = VerifyingKey::from_value(&listed.public_key)
            .map_err(|_| TrustError::OtherKey(own_oin.clone()))?;
        if !listed_key.same_key(&own_key) {
            return Err(TrustError::OtherKey(own_oin));
        }
        let issuers = document
            .app_managers
            .into_iter()
            .map(|m| {
                let key = VerifyingKey::from_value(&m.public_key).map_err(|source| {
                    TrustError::IssuerKey {
                        oin: m.oin.clone(),
                        source,
                    }
                })?;
                Ok(Issuer {
                    oin: m.oin.parse()?,
                    name: m.name,
                    key,
                })
            })
            .collect::<Result<Vec<_>, TrustError>>()?;
        if issuers.is_empty() {
            return Err(TrustError::NoIssuers);
        }
        Ok(Self {
            own_oin,
            key: input.key,
            scope: input.scope.trim().to_string(),
            issuers,
            stelsel_version: document.version,
            demo: false,
        })
    }

    /// The trust marked as that of a demo Stelsel, which only test mode
    /// accepts ([`crate::app`]).
    pub(crate) fn into_demo(mut self) -> Self {
        self.demo = true;
        self
    }

    /// Whether this is the trust of a demo Stelsel.
    pub fn is_demo(&self) -> bool {
        self.demo
    }

    pub fn own_oin(&self) -> &Oin {
        &self.own_oin
    }

    /// The organization's root key, which signs the challenges.
    pub fn key(&self) -> &SigningKey {
        &self.key
    }

    /// The scope a credential must carry.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// The version of the Stelseldocument the trust was read from.
    pub fn stelsel_version(&self) -> u64 {
        self.stelsel_version
    }

    /// The Activatiedienst with OIN `oin` and, when the credential names a
    /// `kid`, a key with that `kid` (§4.3.1, §4.3.3).
    pub fn issuer(&self, oin: &str, kid: Option<&str>) -> Option<&Issuer> {
        self.issuers.iter().find(|i| {
            i.oin.as_str() == oin
                && kid.is_none_or(|k| i.key.jwk().kid.as_deref().is_none_or(|own| own == k))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    const OWN: &str = "00000099000000000001";
    const ISSUER: &str = "00000099000000000002";

    struct Keys {
        beheerder: SigningKey,
        own: SigningKey,
        issuer: SigningKey,
    }

    fn keys() -> Keys {
        Keys {
            beheerder: SigningKey::generate().unwrap(),
            own: SigningKey::generate().unwrap(),
            issuer: SigningKey::generate().unwrap(),
        }
    }

    fn document(k: &Keys, own_key: &SigningKey) -> String {
        k.beheerder
            .sign(
                &json!({
                    "version": 3,
                    "organizations": [{ "oin": OWN, "name": "Gemeente", "discovery_url": "https://x", "public_key": own_key.public_jwk() }],
                    "app_managers": [{ "oin": ISSUER, "name": "Activatiedienst", "discovery_url": "https://y", "public_key": k.issuer.public_jwk() }],
                }),
                &[],
            )
            .unwrap()
    }

    fn build(k: Keys, doc: &str, beheerder: &VerifyingKey) -> Result<Trust, TrustError> {
        Trust::build(TrustInput {
            own_oin: OWN,
            key: k.own,
            scope: "nl.voorbeeld",
            stelseldocument: doc,
            stelselbeheerder: beheerder,
        })
    }

    #[test]
    fn a_signed_stelseldocument_gives_its_activatiediensten() {
        let k = keys();
        let doc = document(&k, &k.own);
        let beheerder = k.beheerder.verifying_key().unwrap();
        let trust = build(k, &doc, &beheerder).unwrap();
        assert_eq!(trust.stelsel_version(), 3);
        assert!(trust.issuer(ISSUER, None).is_some());
        assert!(trust.issuer(OWN, None).is_none());
        assert!(!trust.is_demo());
    }

    #[test]
    fn a_stelseldocument_signed_by_another_is_refused() {
        let k = keys();
        let doc = document(&k, &k.own);
        let other = SigningKey::generate().unwrap().verifying_key().unwrap();
        assert!(matches!(
            build(k, &doc, &other).unwrap_err(),
            TrustError::Signature(JoseError::Signature)
        ));
    }

    #[test]
    fn the_stelseldocument_must_list_this_organization_with_its_own_key() {
        let k = keys();
        let stranger = SigningKey::generate().unwrap();
        let doc = document(&k, &stranger);
        let beheerder = k.beheerder.verifying_key().unwrap();
        assert!(matches!(
            build(k, &doc, &beheerder).unwrap_err(),
            TrustError::OtherKey(_)
        ));
    }

    #[test]
    fn a_kid_picks_the_key_of_the_activatiedienst() {
        let k = keys();
        let mut issuer_jwk = k.issuer.public_jwk().clone();
        issuer_jwk.kid = Some("1".into());
        let doc = k
            .beheerder
            .sign(
                &json!({
                    "version": 1,
                    "organizations": [{ "oin": OWN, "public_key": k.own.public_jwk() }],
                    "app_managers": [{ "oin": ISSUER, "public_key": issuer_jwk }],
                }),
                &[],
            )
            .unwrap();
        let beheerder = k.beheerder.verifying_key().unwrap();
        let trust = build(k, &doc, &beheerder).unwrap();
        assert!(trust.issuer(ISSUER, Some("1")).is_some());
        assert!(trust.issuer(ISSUER, None).is_some());
        assert!(trust.issuer(ISSUER, Some("2")).is_none());
    }

    #[test]
    fn an_oin_has_twenty_digits() {
        assert!("00000099000000000001".parse::<Oin>().is_ok());
        assert!("123".parse::<Oin>().is_err());
        assert!("0000009900000000000x".parse::<Oin>().is_err());
    }
}
