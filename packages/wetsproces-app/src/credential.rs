//! What the app reads from its own credential: whose it is, until when it
//! lasts, and that it is bound to the app's key.
//!
//! The app does not check the Activatiedienst's signature: the server does
//! that on every `CreateSession`, against the Stelseldocument it trusts. The
//! app reads the claims so that it can tell the citizen whose credential it
//! holds and ask for a new one before the server refuses the old one.

use regelrecht_fsc4b::jose::{Jwk, Jws};
use serde::Serialize;
use serde_json::{Map, Value};

/// Why a credential does not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CredentialError {
    #[error("the credential is no JWS: {0}")]
    Token(String),
    #[error("the credential lacks `{0}`")]
    Missing(&'static str),
    #[error("the credential is bound to another key than the app's")]
    OtherKey,
}

/// The citizen a credential names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    pub bsn: String,
    pub name: Option<String>,
}

/// A credential that names a citizen, lasts until `expires_at`, and is
/// bound to the key it was read for.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    token: String,
    holder: Holder,
    expires_at: i64,
}

impl std::fmt::Debug for Credential {
    // The token is what the app shows the server; it stays out of logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

fn text<'a>(claims: &'a Map<String, Value>, name: &'static str) -> Option<&'a str> {
    claims
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

impl Credential {
    /// The credential `token`, which must be bound to the app key `key`.
    ///
    /// # Errors
    ///
    /// When `token` is no JWS, lacks `bsn`, `exp` or `cnf.jwk`, or names
    /// another key.
    pub fn read(token: &str, key: &Jwk) -> Result<Self, CredentialError> {
        let token = token.trim();
        let jws = Jws::unverified(token).map_err(|e| CredentialError::Token(e.to_string()))?;
        let claims = &jws.claims;
        let bound: Jwk = claims
            .get("cnf")
            .and_then(|c| c.get("jwk"))
            .and_then(|j| serde_json::from_value(j.clone()).ok())
            .ok_or(CredentialError::Missing("cnf.jwk"))?;
        if bound.thumbprint() != key.thumbprint() {
            return Err(CredentialError::OtherKey);
        }
        let bsn = text(claims, "bsn").ok_or(CredentialError::Missing("bsn"))?;
        let expires_at = claims
            .get("exp")
            .and_then(Value::as_i64)
            .ok_or(CredentialError::Missing("exp"))?;
        let name = [text(claims, "given_name"), text(claims, "family_name")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Self {
            token: token.to_string(),
            holder: Holder {
                bsn: bsn.to_string(),
                name: (!name.is_empty()).then_some(name),
            },
            expires_at,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn holder(&self) -> &Holder {
        &self.holder
    }

    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }

    /// Whether the credential has lapsed at the Unix time `now`, or lapses
    /// within `margin` seconds: a session started that close to the end
    /// would end before it is used.
    pub fn lapsed(&self, now: i64, margin: i64) -> bool {
        now.saturating_add(margin) >= self.expires_at
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use regelrecht_fsc4b::jose::SigningKey;
    use serde_json::json;

    use super::*;

    fn token(app: &SigningKey, exp: i64) -> String {
        SigningKey::generate()
            .unwrap()
            .sign(
                &json!({
                    "cnf": { "jwk": app.public_jwk() },
                    "bsn": "999990019",
                    "given_name": "Test",
                    "family_name": "Burger",
                    "exp": exp,
                }),
                &[],
            )
            .unwrap()
    }

    #[test]
    fn a_credential_names_its_holder_and_its_end() {
        let app = SigningKey::generate().unwrap();
        let c = Credential::read(&token(&app, 2_000), app.public_jwk()).unwrap();
        assert_eq!(c.holder().bsn, "999990019");
        assert_eq!(c.holder().name.as_deref(), Some("Test Burger"));
        assert!(!c.lapsed(1_000, 60));
        assert!(c.lapsed(1_950, 60));
        assert!(c.lapsed(2_000, 0));
        assert!(!format!("{c:?}").contains(c.token()));
    }

    #[test]
    fn a_credential_for_another_key_is_refused() {
        let app = SigningKey::generate().unwrap();
        let other = SigningKey::generate().unwrap();
        assert_eq!(
            Credential::read(&token(&other, 2_000), app.public_jwk()),
            Err(CredentialError::OtherKey)
        );
        assert!(matches!(
            Credential::read("no.jws", app.public_jwk()),
            Err(CredentialError::Token(_))
        ));
    }
}
