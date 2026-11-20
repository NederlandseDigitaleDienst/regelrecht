//! The JOSE formats FSC voor Burgers speaks (§3 of the specification), on
//! the primitives of `aws-lc-rs`, the cryptographic library the server
//! already links for TLS, and the Concat KDF of RFC 7518 §4.6.2 from the
//! `concat-kdf` crate. This module writes no primitive of its own: it puts
//! signatures, key agreement and AES-GCM together in the compact
//! serialisations of RFC 7515 (JWS) and RFC 7516 (JWE).
//!
//! What it accepts is narrow on purpose, and every choice is a constant
//! here:
//!
//! - signatures: ES256 only ([`JWS_ALG`]), ECDSA over P-256, the one curve
//!   the secure hardware of phones offers (§3.1);
//! - asymmetric encryption: `ECDH-ES` over P-256 with `A256GCM`
//!   ([`JWE_ALG_ECDH_ES`], [`JWE_ENC`]); the post-quantum hybrid of §3.3
//!   is not offered, see the docs page;
//! - symmetric encryption: `dir` with `A256GCM` and a key of
//!   [`AES_KEY_LEN`] bytes (§3.2, §3.4).
//!
//! A token with another `alg` or `enc`, or with a `crit` or `zip` header, is refused
//! before any key touches it, so no request can ask for a weaker algorithm.

use std::fmt;

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use aws_lc_rs::agreement::{self, ECDH_P256};
use aws_lc_rs::encoding::{AsBigEndian, AsDer, EcPrivateKeyBin, PublicKeyX509Der};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use aws_lc_rs::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair,
    ParsedPublicKey,
};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The only signature algorithm accepted and made.
pub const JWS_ALG: &str = "ES256";

/// The key agreement of a JWE to a public key: ECDH-ES, direct.
pub const JWE_ALG_ECDH_ES: &str = "ECDH-ES";

/// The key management of a JWE under a shared AES key.
pub const JWE_ALG_DIR: &str = "dir";

/// The only content encryption accepted and made.
pub const JWE_ENC: &str = "A256GCM";

/// The length of an AES key for `A256GCM`: 256 bits.
pub const AES_KEY_LEN: usize = 32;

/// The length of a P-256 coordinate in a JWK.
const COORDINATE_LEN: usize = 32;

/// The length of a fixed ES256 signature: `r || s`.
const SIGNATURE_LEN: usize = 64;

/// The length of an AES-GCM tag.
const TAG_LEN: usize = 16;

/// The longest compact token this module reads. Every token of the
/// protocol is far smaller; a longer one is refused before it is decoded.
pub const MAX_TOKEN_LEN: usize = 256 * 1024;

/// Why a token or key is refused. The variants say which check failed, for
/// the log and the tests; an answer to a client says less.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JoseError {
    #[error("malformed: {0}")]
    Malformed(&'static str),
    #[error("the algorithm `{0}` is not accepted")]
    Algorithm(String),
    #[error("the key is not a valid P-256 key")]
    Key,
    #[error("the signature does not verify")]
    Signature,
    #[error("the token does not decrypt")]
    Decrypt,
    #[error("the cryptographic library failed")]
    Crypto,
}

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64url(text: &str, what: &'static str) -> Result<Vec<u8>, JoseError> {
    URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| JoseError::Malformed(what))
}

/// Standard base64 with padding, as the protocol writes nonces, the
/// session key and the ephemeral key (§4.3.4.2.2.4).
pub fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// The bytes of standard base64 with padding.
///
/// # Errors
///
/// [`JoseError::Malformed`] when `text` is no such base64.
pub fn unb64(text: &str, what: &'static str) -> Result<Vec<u8>, JoseError> {
    STANDARD
        .decode(text)
        .map_err(|_| JoseError::Malformed(what))
}

/// `n` bytes from the operating system's CSPRNG.
///
/// # Errors
///
/// [`JoseError::Crypto`] when the generator fails.
pub fn random_bytes(n: usize) -> Result<Vec<u8>, JoseError> {
    let mut bytes = vec![0u8; n];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| JoseError::Crypto)?;
    Ok(bytes)
}

/// A public P-256 key as a JWK (RFC 7517), with what it may carry beside
/// its coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alg: Option<String>,
    #[serde(default, rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<String>,
}

impl Jwk {
    /// The uncompressed SEC1 point of the key, after checking that it is a
    /// P-256 key of the right size, for the use `purpose` (`sig` or `enc`):
    /// a key whose `use` names the other one is refused.
    fn point(&self, purpose: &str) -> Result<Vec<u8>, JoseError> {
        if self.kty != "EC" || self.crv != "P-256" {
            return Err(JoseError::Key);
        }
        if self.use_.as_deref().is_some_and(|u| u != purpose) {
            return Err(JoseError::Key);
        }
        if self
            .alg
            .as_deref()
            .is_some_and(|a| a != JWS_ALG && a != JWE_ALG_ECDH_ES)
        {
            return Err(JoseError::Key);
        }
        let x = unb64url(&self.x, "a JWK coordinate")?;
        let y = unb64url(&self.y, "a JWK coordinate")?;
        if x.len() != COORDINATE_LEN || y.len() != COORDINATE_LEN {
            return Err(JoseError::Key);
        }
        let mut point = Vec::with_capacity(1 + 2 * COORDINATE_LEN);
        point.push(0x04);
        point.extend_from_slice(&x);
        point.extend_from_slice(&y);
        Ok(point)
    }

    /// The JWK of an uncompressed SEC1 P-256 point.
    fn from_point(point: &[u8]) -> Result<Self, JoseError> {
        if point.len() != 1 + 2 * COORDINATE_LEN || point.first() != Some(&0x04) {
            return Err(JoseError::Key);
        }
        let (x, y) = point[1..].split_at(COORDINATE_LEN);
        Ok(Self {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: b64url(x),
            y: b64url(y),
            kid: None,
            alg: None,
            use_: None,
        })
    }

    /// The JWK thumbprint of RFC 7638 (SHA-256), base64url: one name for
    /// the key, whatever else the JWK carries.
    pub fn thumbprint(&self) -> String {
        // The members in lexicographic order, without whitespace (§3.2).
        let canonical = format!(
            r#"{{"crv":"{}","kty":"{}","x":"{}","y":"{}"}}"#,
            self.crv, self.kty, self.x, self.y
        );
        b64url(&Sha256::digest(canonical.as_bytes()))
    }
}

/// A public ES256 key that has been checked to be a point on P-256.
pub struct VerifyingKey {
    key: ParsedPublicKey,
    jwk: Jwk,
}

impl fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifyingKey")
            .field("thumbprint", &self.jwk.thumbprint())
            .finish()
    }
}

impl VerifyingKey {
    /// The key in `jwk`, checked: a P-256 key whose coordinates lie on the
    /// curve (§4.3.1.1 step 5).
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] for any other key.
    pub fn from_jwk(jwk: &Jwk) -> Result<Self, JoseError> {
        let key = ParsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, jwk.point("sig")?)
            .map_err(|_| JoseError::Key)?;
        Ok(Self {
            key,
            jwk: jwk.clone(),
        })
    }

    /// The key in a JSON value holding a JWK.
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] when the value is no P-256 JWK.
    pub fn from_value(value: &Value) -> Result<Self, JoseError> {
        let jwk: Jwk = serde_json::from_value(value.clone()).map_err(|_| JoseError::Key)?;
        Self::from_jwk(&jwk)
    }

    pub fn jwk(&self) -> &Jwk {
        &self.jwk
    }

    /// The RFC 7638 thumbprint of the key.
    pub fn thumbprint(&self) -> String {
        self.jwk.thumbprint()
    }

    /// Whether `other` is the same public key.
    pub fn same_key(&self, other: &Self) -> bool {
        self.key.as_ref() == other.key.as_ref()
    }

    /// The claims of `jws` when this key signed it.
    ///
    /// # Errors
    ///
    /// When the token is malformed, uses another algorithm than
    /// [`JWS_ALG`] or a `crit` header, or the signature does not verify.
    pub fn verify(&self, jws: &str) -> Result<Jws, JoseError> {
        let parts = Jws::parse(jws)?;
        let signature = unb64url(parts.signature, "the JWS signature")?;
        if signature.len() != SIGNATURE_LEN {
            return Err(JoseError::Signature);
        }
        self.key
            .verify_sig(parts.signing_input.as_bytes(), &signature)
            .map_err(|_| JoseError::Signature)?;
        Ok(parts.decoded)
    }
}

/// A JWS read apart, header and claims, verified or not: [`Jws::unverified`]
/// gives one whose signature nobody checked, to find the key that should
/// have made it.
#[derive(Debug, Clone)]
pub struct Jws {
    pub header: Map<String, Value>,
    pub claims: Map<String, Value>,
}

struct JwsParts<'a> {
    signing_input: &'a str,
    signature: &'a str,
    decoded: Jws,
}

/// The JSON object a base64url segment holds.
fn object(segment: &str, what: &'static str) -> Result<Map<String, Value>, JoseError> {
    let bytes = unb64url(segment, what)?;
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(JoseError::Malformed(what)),
    }
}

/// Refuse a header that asks for anything but `alg` (and `enc`) as given.
fn check_header(
    header: &Map<String, Value>,
    alg: &[&str],
    enc: Option<&str>,
) -> Result<(), JoseError> {
    if header.contains_key("crit") {
        return Err(JoseError::Malformed("a `crit` header"));
    }
    // Compression before encryption is not offered: it is nowhere in the
    // standard, and it leaks the plaintext's length pattern.
    if header.contains_key("zip") {
        return Err(JoseError::Malformed("a `zip` header"));
    }
    match header.get("alg") {
        Some(Value::String(a)) if alg.contains(&a.as_str()) => {}
        Some(Value::String(a)) => return Err(JoseError::Algorithm(a.clone())),
        _ => return Err(JoseError::Malformed("a header without `alg`")),
    }
    if let Some(enc) = enc {
        match header.get("enc") {
            Some(Value::String(e)) if e == enc => {}
            Some(Value::String(e)) => return Err(JoseError::Algorithm(e.clone())),
            _ => return Err(JoseError::Malformed("a JWE header without `enc`")),
        }
    }
    Ok(())
}

impl Jws {
    fn parse(jws: &str) -> Result<JwsParts<'_>, JoseError> {
        if jws.len() > MAX_TOKEN_LEN {
            return Err(JoseError::Malformed("a token that is too long"));
        }
        let mut segments = jws.split('.');
        let (Some(header), Some(claims), Some(signature), None) = (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
        ) else {
            return Err(JoseError::Malformed("a JWS has three segments"));
        };
        let decoded = Jws {
            header: object(header, "the JWS header")?,
            claims: object(claims, "the JWS claims")?,
        };
        check_header(&decoded.header, &[JWS_ALG], None)?;
        // `header.claims`, a prefix of `jws` ending before the second dot.
        let signing_input = &jws[..header.len() + 1 + claims.len()];
        Ok(JwsParts {
            signing_input,
            signature,
            decoded,
        })
    }

    /// The header and claims of `jws`, without checking its signature, for
    /// finding the key that should have made it (§4.3.4.1, "parse the
    /// header and body without verifying"). Nothing read here may be
    /// trusted until [`VerifyingKey::verify`] passed.
    ///
    /// # Errors
    ///
    /// When the token is malformed or uses another algorithm than
    /// [`JWS_ALG`].
    pub fn unverified(jws: &str) -> Result<Self, JoseError> {
        Ok(Self::parse(jws)?.decoded)
    }

    /// The text claim `name`.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.claims.get(name).and_then(Value::as_str)
    }

    /// The text header `name`.
    pub fn header_text(&self, name: &str) -> Option<&str> {
        self.header.get(name).and_then(Value::as_str)
    }
}

/// An ES256 private key, with its public JWK.
pub struct SigningKey {
    pair: EcdsaKeyPair,
    jwk: Jwk,
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("thumbprint", &self.jwk.thumbprint())
            .finish_non_exhaustive()
    }
}

impl SigningKey {
    fn from_pair(pair: EcdsaKeyPair) -> Result<Self, JoseError> {
        let jwk = Jwk::from_point(pair.public_key().as_ref())?;
        Ok(Self { pair, jwk })
    }

    /// A fresh key from the CSPRNG.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the library fails.
    pub fn generate() -> Result<Self, JoseError> {
        let pair = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING)
            .map_err(|_| JoseError::Crypto)?;
        Self::from_pair(pair)
    }

    /// The key in a PKCS#8 document, DER.
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] when it holds no P-256 key.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self, JoseError> {
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der)
            .map_err(|_| JoseError::Key)?;
        Self::from_pair(pair)
    }

    /// The key in a PEM `PRIVATE KEY` block (PKCS#8), as `openssl genpkey`
    /// writes it.
    ///
    /// # Errors
    ///
    /// [`JoseError::Malformed`] when there is no such block,
    /// [`JoseError::Key`] when it holds no P-256 key.
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self, JoseError> {
        const BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
        const END: &str = "-----END PRIVATE KEY-----";
        let start = pem
            .find(BEGIN)
            .ok_or(JoseError::Malformed("no PEM `PRIVATE KEY` block"))?
            + BEGIN.len();
        let end = pem[start..]
            .find(END)
            .ok_or(JoseError::Malformed("an unterminated PEM block"))?
            + start;
        let body: String = pem[start..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let der = unb64(&body, "the PEM body")?;
        Self::from_pkcs8_der(&der)
    }

    /// The key as PKCS#8 PEM, for a client that keeps it.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the library fails.
    pub fn to_pkcs8_pem(&self) -> Result<String, JoseError> {
        let der = self.pair.to_pkcs8v1().map_err(|_| JoseError::Crypto)?;
        let body = b64(der.as_ref());
        let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");
        for line in body.as_bytes().chunks(64) {
            pem.push_str(&String::from_utf8_lossy(line));
            pem.push('\n');
        }
        pem.push_str("-----END PRIVATE KEY-----\n");
        Ok(pem)
    }

    /// The public key, as a JWK.
    pub fn public_jwk(&self) -> &Jwk {
        &self.jwk
    }

    /// The public key, as a key that verifies.
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] when the library rejects its own key.
    pub fn verifying_key(&self) -> Result<VerifyingKey, JoseError> {
        VerifyingKey::from_jwk(&self.jwk)
    }

    /// A compact JWS of `claims`, with `typ: jwt`, `alg: ES256` and the
    /// headers in `extra` (such as a `kid`).
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when signing fails.
    pub fn sign(&self, claims: &Value, extra: &[(&str, &str)]) -> Result<String, JoseError> {
        sign_jws(self, claims, extra)
    }
}

/// What signs with an ES256 key: a key in this process ([`SigningKey`]), or
/// one an app keeps in secure hardware, which signs without handing the
/// private key out. The JWS around the signature is [`sign_jws`]'s work.
pub trait Es256Signer: Send + Sync {
    /// The public key, as a JWK.
    fn public_jwk(&self) -> &Jwk;

    /// The ES256 signature over `input`: `r` and `s`, 32 bytes each, as
    /// RFC 7518 §3.4 puts them in a JWS.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when signing fails.
    fn sign_es256(&self, input: &[u8]) -> Result<Vec<u8>, JoseError>;
}

impl Es256Signer for SigningKey {
    fn public_jwk(&self) -> &Jwk {
        &self.jwk
    }

    fn sign_es256(&self, input: &[u8]) -> Result<Vec<u8>, JoseError> {
        let signature = self
            .pair
            .sign(&SystemRandom::new(), input)
            .map_err(|_| JoseError::Crypto)?;
        Ok(signature.as_ref().to_vec())
    }
}

/// A compact JWS of `claims` by `signer`, with `typ: jwt`, `alg: ES256` and
/// the headers in `extra` (such as a `kid`).
///
/// # Errors
///
/// [`JoseError::Crypto`] when signing fails or the signature is not
/// 64 bytes.
pub fn sign_jws(
    signer: &dyn Es256Signer,
    claims: &Value,
    extra: &[(&str, &str)],
) -> Result<String, JoseError> {
    let mut header = Map::new();
    header.insert("typ".into(), Value::from("jwt"));
    header.insert("alg".into(), Value::from(JWS_ALG));
    for (name, value) in extra {
        header.insert((*name).into(), Value::from(*value));
    }
    let input = format!(
        "{}.{}",
        b64url(Value::Object(header).to_string().as_bytes()),
        b64url(claims.to_string().as_bytes())
    );
    let signature = signer.sign_es256(input.as_bytes())?;
    if signature.len() != SIGNATURE_LEN {
        return Err(JoseError::Crypto);
    }
    Ok(format!("{input}.{}", b64url(&signature)))
}

/// A symmetric `A256GCM` key, such as the session key of a CreateSession.
#[derive(Clone)]
pub struct AesKey([u8; AES_KEY_LEN]);

impl fmt::Debug for AesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AesKey(<redacted>)")
    }
}

impl AesKey {
    /// The key in `bytes`, which must be [`AES_KEY_LEN`] long.
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] for another length.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, JoseError> {
        bytes.try_into().map(Self).map_err(|_| JoseError::Key)
    }

    /// A fresh key from the CSPRNG.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the generator fails.
    pub fn generate() -> Result<Self, JoseError> {
        Self::from_slice(&random_bytes(AES_KEY_LEN)?)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// `plaintext` as a compact JWE under this key, `alg: dir`.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when encryption fails.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String, JoseError> {
        let header = serde_json::json!({ "alg": JWE_ALG_DIR, "enc": JWE_ENC });
        seal(&self.0, &header, plaintext)
    }

    /// The plaintext of a compact JWE made under this key, `alg: dir`.
    ///
    /// # Errors
    ///
    /// When the token is malformed, names another algorithm, or does not
    /// decrypt under this key.
    pub fn decrypt(&self, jwe: &str) -> Result<Vec<u8>, JoseError> {
        let parts = JweParts::parse(jwe, JWE_ALG_DIR)?;
        if !parts.encrypted_key.is_empty() {
            return Err(JoseError::Malformed("a `dir` JWE carries no encrypted key"));
        }
        open(&self.0, &parts)
    }
}

/// The five segments of a compact JWE, decoded.
struct JweParts<'a> {
    protected: &'a str,
    header: Map<String, Value>,
    encrypted_key: Vec<u8>,
    iv: Vec<u8>,
    ciphertext: Vec<u8>,
    tag: Vec<u8>,
}

impl<'a> JweParts<'a> {
    fn parse(jwe: &'a str, alg: &str) -> Result<Self, JoseError> {
        if jwe.len() > MAX_TOKEN_LEN {
            return Err(JoseError::Malformed("a token that is too long"));
        }
        let segments: Vec<&str> = jwe.split('.').collect();
        let [protected, key, iv, ciphertext, tag] = segments.as_slice() else {
            return Err(JoseError::Malformed("a JWE has five segments"));
        };
        let header = object(protected, "the JWE header")?;
        check_header(&header, &[alg], Some(JWE_ENC))?;
        let parts = Self {
            protected,
            header,
            encrypted_key: unb64url(key, "the JWE key")?,
            iv: unb64url(iv, "the JWE iv")?,
            ciphertext: unb64url(ciphertext, "the JWE ciphertext")?,
            tag: unb64url(tag, "the JWE tag")?,
        };
        if parts.iv.len() != NONCE_LEN || parts.tag.len() != TAG_LEN {
            return Err(JoseError::Malformed("a JWE iv or tag of the wrong size"));
        }
        Ok(parts)
    }
}

/// `plaintext` under `key` with the protected `header`, as a compact JWE
/// without an encrypted key.
fn seal(key: &[u8], header: &Value, plaintext: &[u8]) -> Result<String, JoseError> {
    let protected = b64url(header.to_string().as_bytes());
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).map_err(|_| JoseError::Key)?);
    let iv = random_bytes(NONCE_LEN)?;
    let nonce = Nonce::try_assume_unique_for_key(&iv).map_err(|_| JoseError::Crypto)?;
    let mut buffer = plaintext.to_vec();
    let tag = key
        .seal_in_place_separate_tag(nonce, Aad::from(protected.as_bytes()), &mut buffer)
        .map_err(|_| JoseError::Crypto)?;
    Ok(format!(
        "{protected}..{}.{}.{}",
        b64url(&iv),
        b64url(&buffer),
        b64url(tag.as_ref())
    ))
}

/// The plaintext of `parts` under `key`; the protected header is the
/// additional data, as RFC 7516 §5.2 says.
fn open(key: &[u8], parts: &JweParts<'_>) -> Result<Vec<u8>, JoseError> {
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).map_err(|_| JoseError::Key)?);
    let nonce = Nonce::try_assume_unique_for_key(&parts.iv).map_err(|_| JoseError::Decrypt)?;
    let mut buffer = parts.ciphertext.clone();
    buffer.extend_from_slice(&parts.tag);
    let plaintext = key
        .open_in_place(nonce, Aad::from(parts.protected.as_bytes()), &mut buffer)
        .map_err(|_| JoseError::Decrypt)?;
    Ok(plaintext.to_vec())
}

/// The content key of ECDH-ES direct (RFC 7518 §4.6.2) for [`JWE_ENC`].
fn content_key(z: &[u8], apu: &[u8], apv: &[u8]) -> Result<[u8; AES_KEY_LEN], JoseError> {
    let mut key = [0u8; AES_KEY_LEN];
    concat_kdf_into(z, JWE_ENC, apu, apv, &mut key)?;
    Ok(key)
}

/// The Concat KDF of RFC 7518 §4.6.2 over the shared secret `z`, with `enc`
/// as the algorithm id and the length of `key` as the key length, into
/// `key`.
fn concat_kdf_into(
    z: &[u8],
    enc: &str,
    apu: &[u8],
    apv: &[u8],
    key: &mut [u8],
) -> Result<(), JoseError> {
    let length_prefixed = |data: &[u8]| -> Result<Vec<u8>, JoseError> {
        let len = u32::try_from(data.len()).map_err(|_| JoseError::Malformed("apu or apv"))?;
        let mut out = len.to_be_bytes().to_vec();
        out.extend_from_slice(data);
        Ok(out)
    };
    let mut other_info = length_prefixed(enc.as_bytes())?;
    other_info.extend(length_prefixed(apu)?);
    other_info.extend(length_prefixed(apv)?);
    // SuppPubInfo: the key length in bits, as a 32-bit big-endian number.
    let bits = key
        .len()
        .checked_mul(8)
        .and_then(|b| u32::try_from(b).ok())
        .ok_or(JoseError::Crypto)?;
    other_info.extend_from_slice(&bits.to_be_bytes());
    concat_kdf::derive_key_into::<Sha256>(z, &other_info, key).map_err(|_| JoseError::Crypto)
}

/// The optional `apu` or `apv` header, decoded.
fn party_info(header: &Map<String, Value>, name: &str) -> Result<Vec<u8>, JoseError> {
    match header.get(name) {
        None => Ok(Vec::new()),
        Some(Value::String(s)) => unb64url(s, "an `apu` or `apv` header"),
        Some(_) => Err(JoseError::Malformed("an `apu` or `apv` header")),
    }
}

/// An ephemeral ECDH key pair of the organization (§4.3.4.2.2.4): its
/// public half goes into the challenge, its private half decrypts the
/// presentation once.
pub struct EphemeralKey {
    key: agreement::PrivateKey,
}

impl fmt::Debug for EphemeralKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EphemeralKey(<redacted>)")
    }
}

impl EphemeralKey {
    /// A fresh key pair from the CSPRNG.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the library fails.
    pub fn generate() -> Result<Self, JoseError> {
        agreement::PrivateKey::generate(&ECDH_P256)
            .map(|key| Self { key })
            .map_err(|_| JoseError::Crypto)
    }

    /// The private scalar, big-endian, to keep beside the nonce until the
    /// app answers.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the library fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, JoseError> {
        let bin: EcPrivateKeyBin<'_> = self.key.as_be_bytes().map_err(|_| JoseError::Crypto)?;
        Ok(bin.as_ref().to_vec())
    }

    /// The key pair from its private scalar.
    ///
    /// # Errors
    ///
    /// [`JoseError::Key`] when the bytes are no P-256 scalar.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, JoseError> {
        agreement::PrivateKey::from_private_key(&ECDH_P256, bytes)
            .map(|key| Self { key })
            .map_err(|_| JoseError::Key)
    }

    /// The public key as DER (X.509 SubjectPublicKeyInfo), as the challenge
    /// carries it, base64.
    ///
    /// # Errors
    ///
    /// [`JoseError::Crypto`] when the library fails.
    pub fn public_der_b64(&self) -> Result<String, JoseError> {
        let public = self
            .key
            .compute_public_key()
            .map_err(|_| JoseError::Crypto)?;
        let der: PublicKeyX509Der<'_> = public.as_der().map_err(|_| JoseError::Crypto)?;
        Ok(b64(der.as_ref()))
    }

    /// The plaintext of a compact JWE made to this key with `ECDH-ES` and
    /// `A256GCM`.
    ///
    /// # Errors
    ///
    /// When the token is malformed, names another algorithm, carries no
    /// valid P-256 `epk`, or does not decrypt.
    pub fn decrypt(&self, jwe: &str) -> Result<Vec<u8>, JoseError> {
        let parts = JweParts::parse(jwe, JWE_ALG_ECDH_ES)?;
        if !parts.encrypted_key.is_empty() {
            return Err(JoseError::Malformed(
                "an ECDH-ES JWE carries no encrypted key",
            ));
        }
        let epk = parts
            .header
            .get("epk")
            .ok_or(JoseError::Malformed("an ECDH-ES JWE without `epk`"))?;
        let epk: Jwk = serde_json::from_value(epk.clone()).map_err(|_| JoseError::Key)?;
        let peer = agreement::UnparsedPublicKey::new(&ECDH_P256, epk.point("enc")?);
        let apu = party_info(&parts.header, "apu")?;
        let apv = party_info(&parts.header, "apv")?;
        let key = agreement::agree(&self.key, &peer, JoseError::Key, |z| {
            content_key(z, &apu, &apv)
        })?;
        open(&key, &parts)
    }
}

/// `plaintext` as a compact JWE to the P-256 public key in `der` (X.509
/// SubjectPublicKeyInfo), with `ECDH-ES` and `A256GCM`: what an app does
/// with the challenge's ephemeral key (§4.3.4.2.2.7).
///
/// # Errors
///
/// [`JoseError::Key`] when `der` holds no P-256 key, and
/// [`JoseError::Crypto`] when the library fails.
pub fn encrypt_to_der(der: &[u8], plaintext: &[u8]) -> Result<String, JoseError> {
    let peer = agreement::UnparsedPublicKey::new(&ECDH_P256, der);
    let ephemeral = agreement::PrivateKey::generate(&ECDH_P256).map_err(|_| JoseError::Crypto)?;
    let public = ephemeral
        .compute_public_key()
        .map_err(|_| JoseError::Crypto)?;
    let epk = Jwk::from_point(public.as_ref())?;
    let key = agreement::agree(&ephemeral, peer, JoseError::Key, |z| {
        content_key(z, &[], &[])
    })?;
    let header = serde_json::json!({ "alg": JWE_ALG_ECDH_ES, "enc": JWE_ENC, "epk": epk });
    seal(&key, &header, plaintext)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The keys of RFC 7518 Appendix C, published there as an example: Bob's
    /// static key and Alice's ephemeral one. They protect nothing.
    const RFC_BOB_D: &str = "VEmDZpDXXK8p8N0Cndsxs924q6nS1RXFASRl6BfUqdw";
    const RFC_ALICE_EPK: &str = r#"{"kty":"EC","crv":"P-256","x":"gI0GAILBdu7T53akrFmMyGcsF3n5dO7MmwNBHKW5SV0","y":"SLW_xSffzlPWrHEVI30DHM_4egVwt3NQqeUD7nMFpps"}"#;

    fn rfc_bob() -> EphemeralKey {
        EphemeralKey::from_bytes(&unb64url(RFC_BOB_D, "d").unwrap()).unwrap()
    }

    #[test]
    fn key_agreement_and_the_concat_kdf_match_rfc_7518_appendix_c() {
        // Appendix C derives a key for A128GCM with apu "Alice" and apv
        // "Bob"; the code path is the one `decrypt` takes, with that `enc`
        // and key length.
        let alice: Jwk = serde_json::from_str(RFC_ALICE_EPK).unwrap();
        let peer = agreement::UnparsedPublicKey::new(&ECDH_P256, alice.point("enc").unwrap());
        let mut key = [0u8; 16];
        agreement::agree(&rfc_bob().key, peer, JoseError::Key, |z| {
            concat_kdf_into(z, "A128GCM", b"Alice", b"Bob", &mut key)
        })
        .unwrap();
        assert_eq!(b64url(&key), "VqqN6vgjbSBcIijNcacQGg");
    }

    #[test]
    fn a_jwe_made_by_another_implementation_decrypts() {
        // Made with panva/jose 5.9.6 (`CompactEncrypt`, ECDH-ES, A256GCM) to
        // Bob's public key of RFC 7518 Appendix C.
        let jwe = "eyJhbGciOiJFQ0RILUVTIiwiZW5jIjoiQTI1NkdDTSIsImVwayI6eyJ4IjoiMFlMS3hzQ0pyajI1UTF3alNTZ1J5QjFNcXVIZ3Z6ZWp5cWlUSmlnQUEyOCIsImNydiI6IlAtMjU2Iiwia3R5IjoiRUMiLCJ5IjoiM29ITzZoX3BOamNNZm1xUFNCb2wzNWRNOTEwaDRrV3hFMUdHTGxQS19hcyJ9fQ..xvZPWdfEjzh1XHiR.akhWR-HH55J-nHUuqfHZ3WhUNBeu-UBP.G7-2h7zfmOu4B0bXitLzYg";
        assert_eq!(
            rfc_bob().decrypt(jwe).unwrap(),
            br#"{"interop":"panva/jose"}"#
        );
        // panva/jose, `dir` and A256GCM under the key 0, 1, ..., 31.
        let dir = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..pJNbCxUHtvUfMwnJ.xkxscLpKqlKicNRY.VHrdMwCA0-0k_7j0JA0xiw";
        let key: Vec<u8> = (0u8..32).collect();
        assert_eq!(
            AesKey::from_slice(&key).unwrap().decrypt(dir).unwrap(),
            br#"{"dir":true}"#
        );
        // panva/jose, an ES256 JWS and its public key.
        let jws = "eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJ4In0.8YphPVp0HC167dMVYuvTaFcf5XuSBBXJa9ae-5yHa_7CIV2v0TbJFb8JHB84YHeF0RSx-Cd1uM0qoKRFQ9R31Q";
        let public = json!({
            "kty": "EC", "crv": "P-256",
            "x": "CwwzmkN7xLORvytlmxGT80wLhuaJlzULkWLQP81Xj0o",
            "y": "XGFCJMYwzxVVoX_Yb-ueiG2sj-NGlRqS8L47wMp9sdQ",
        });
        let read = VerifyingKey::from_value(&public)
            .unwrap()
            .verify(jws)
            .unwrap();
        assert_eq!(read.text("sub"), Some("x"));
    }

    #[test]
    fn a_key_marked_for_the_other_use_is_refused() {
        let mut jwk = SigningKey::generate().unwrap().public_jwk().clone();
        jwk.use_ = Some("enc".into());
        assert_eq!(VerifyingKey::from_jwk(&jwk).unwrap_err(), JoseError::Key);
        jwk.use_ = Some("sig".into());
        assert!(VerifyingKey::from_jwk(&jwk).is_ok());
    }

    #[test]
    fn a_jws_verifies_under_its_key_only() {
        let key = SigningKey::generate().unwrap();
        let other = SigningKey::generate().unwrap();
        let jws = key.sign(&json!({ "sub": "x" }), &[("kid", "1")]).unwrap();
        let read = key.verifying_key().unwrap().verify(&jws).unwrap();
        assert_eq!(read.text("sub"), Some("x"));
        assert_eq!(read.header_text("kid"), Some("1"));
        assert_eq!(
            other.verifying_key().unwrap().verify(&jws).unwrap_err(),
            JoseError::Signature
        );
    }

    #[test]
    fn a_jws_with_another_algorithm_is_refused_before_its_signature() {
        let header = b64url(br#"{"alg":"none"}"#);
        let claims = b64url(br#"{"sub":"x"}"#);
        let key = SigningKey::generate().unwrap().verifying_key().unwrap();
        assert_eq!(
            key.verify(&format!("{header}.{claims}.")).unwrap_err(),
            JoseError::Algorithm("none".into())
        );
        let header = b64url(br#"{"alg":"ES256","crit":["exp"]}"#);
        assert!(matches!(
            key.verify(&format!("{header}.{claims}.")).unwrap_err(),
            JoseError::Malformed(_)
        ));
    }

    #[test]
    fn a_jwk_off_the_curve_is_refused() {
        let mut jwk = SigningKey::generate().unwrap().public_jwk().clone();
        jwk.y = b64url(&[7u8; 32]);
        assert_eq!(VerifyingKey::from_jwk(&jwk).unwrap_err(), JoseError::Key);
        let mut jwk = SigningKey::generate().unwrap().public_jwk().clone();
        jwk.crv = "P-384".into();
        assert_eq!(VerifyingKey::from_jwk(&jwk).unwrap_err(), JoseError::Key);
    }

    #[test]
    fn a_jwe_to_the_ephemeral_key_decrypts_with_it_only() {
        let ephemeral = EphemeralKey::generate().unwrap();
        let der = unb64(&ephemeral.public_der_b64().unwrap(), "der").unwrap();
        let jwe = encrypt_to_der(&der, b"presentation").unwrap();
        // Kept and read back, as between two instances.
        let restored = EphemeralKey::from_bytes(&ephemeral.to_bytes().unwrap()).unwrap();
        assert_eq!(restored.decrypt(&jwe).unwrap(), b"presentation");
        let other = EphemeralKey::generate().unwrap();
        assert_eq!(other.decrypt(&jwe).unwrap_err(), JoseError::Decrypt);
    }

    #[test]
    fn a_jwe_under_a_session_key_decrypts_with_it_only_and_untampered() {
        let key = AesKey::generate().unwrap();
        let jwe = key.encrypt(b"{}").unwrap();
        assert_eq!(key.decrypt(&jwe).unwrap(), b"{}");
        assert_eq!(
            AesKey::generate().unwrap().decrypt(&jwe).unwrap_err(),
            JoseError::Decrypt
        );
        let mut segments: Vec<String> = jwe.split('.').map(str::to_string).collect();
        let mut ciphertext = unb64url(&segments[3], "c").unwrap();
        ciphertext[0] ^= 1;
        segments[3] = b64url(&ciphertext);
        assert_eq!(
            key.decrypt(&segments.join(".")).unwrap_err(),
            JoseError::Decrypt
        );
        // A header changed after sealing breaks the additional data.
        segments = jwe.split('.').map(str::to_string).collect();
        segments[0] = b64url(br#"{"alg":"dir","enc":"A256GCM","kid":"x"}"#);
        assert_eq!(
            key.decrypt(&segments.join(".")).unwrap_err(),
            JoseError::Decrypt
        );
    }

    #[test]
    fn a_jwe_that_asks_for_another_algorithm_is_refused() {
        let key = AesKey::generate().unwrap();
        let header = json!({ "alg": "dir", "enc": "A128GCM" });
        let jwe = seal(key.as_bytes(), &header, b"x").unwrap();
        assert_eq!(
            key.decrypt(&jwe).unwrap_err(),
            JoseError::Algorithm("A128GCM".into())
        );
        let ephemeral = EphemeralKey::generate().unwrap();
        assert_eq!(
            ephemeral.decrypt(&key.encrypt(b"x").unwrap()).unwrap_err(),
            JoseError::Algorithm("dir".into())
        );
        let zipped = json!({ "alg": "dir", "enc": "A256GCM", "zip": "DEF" });
        let jwe = seal(key.as_bytes(), &zipped, b"x").unwrap();
        assert!(matches!(
            key.decrypt(&jwe).unwrap_err(),
            JoseError::Malformed(_)
        ));
    }

    #[test]
    fn a_private_key_survives_pem() {
        let key = SigningKey::generate().unwrap();
        let again = SigningKey::from_pkcs8_pem(&key.to_pkcs8_pem().unwrap()).unwrap();
        assert_eq!(key.public_jwk(), again.public_jwk());
    }

    #[test]
    fn the_thumbprint_matches_rfc_7638_shape() {
        // RFC 7638 §3.1 is an RSA key; for EC the members are crv, kty, x, y.
        let jwk = SigningKey::generate().unwrap().public_jwk().clone();
        let mut with_kid = jwk.clone();
        with_kid.kid = Some("1".into());
        assert_eq!(jwk.thumbprint(), with_kid.thumbprint());
        assert_eq!(jwk.thumbprint().len(), 43);
    }
}
