//! Authenticated encryption for secrets that leave the server.
//!
//! Per-user GitHub OAuth tokens are deliberately not persisted anywhere
//! server-side; they live in a browser cookie (see [`crate::github_oauth`]).
//! That cookie crosses the trust boundary on every request, so it must be
//! opaque and unforgeable. This module seals it with ChaCha20-Poly1305 (AEAD)
//! under a single application key supplied via `GITHUB_TOKEN_ENC_KEY` (a
//! base64-encoded 32-byte key): the client can neither read the token nor
//! craft a valid blob, and tampering fails the Poly1305 tag check. The sealed
//! blob is `nonce (12 bytes) || ciphertext+tag`, self-describing, with a fresh
//! random nonce per seal.
//!
//! This is deliberately a thin, purpose-built helper rather than a general
//! crypto layer: one key, one algorithm, one blob format.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

/// ChaCha20-Poly1305 nonce length in bytes.
const NONCE_LEN: usize = 12;

/// Sealing/opening helper bound to one application key.
#[derive(Clone)]
pub struct TokenCipher {
    cipher: ChaCha20Poly1305,
}

impl TokenCipher {
    /// Build a cipher from a base64-encoded 32-byte key.
    ///
    /// Returns a human-readable error (safe to log — it never echoes key
    /// material) when the value is not valid base64 or not exactly 32 bytes.
    pub fn from_base64_key(b64: &str) -> Result<Self, String> {
        let bytes = STANDARD
            .decode(b64.trim())
            .map_err(|e| format!("GITHUB_TOKEN_ENC_KEY is not valid base64: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!(
                "GITHUB_TOKEN_ENC_KEY must decode to 32 bytes, got {}",
                bytes.len()
            ));
        }
        // `new_from_slice` validates the length itself; the explicit check
        // above just yields a clearer message than the crate's generic error.
        let cipher = ChaCha20Poly1305::new_from_slice(&bytes)
            .map_err(|_| "GITHUB_TOKEN_ENC_KEY must be a 32-byte key".to_string())?;
        Ok(Self { cipher })
    }

    /// Seal `plaintext` into `nonce || ciphertext+tag`.
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from(nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| "encryption failed".to_string())?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Open a `nonce || ciphertext+tag` blob produced by [`encrypt`].
    ///
    /// Fails (rather than returning garbage) if the blob was truncated,
    /// tampered with, or sealed under a different key — the Poly1305 tag is
    /// verified before any plaintext is returned.
    ///
    /// [`encrypt`]: TokenCipher::encrypt
    pub fn decrypt(&self, blob: &[u8]) -> Result<String, String> {
        if blob.len() < NONCE_LEN {
            return Err("ciphertext too short to contain a nonce".to_string());
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce_arr: [u8; NONCE_LEN] = nonce_bytes
            .try_into()
            .map_err(|_| "invalid nonce length".to_string())?;
        let nonce = Nonce::from(nonce_arr);
        let plaintext = self
            .cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| "decryption failed (wrong key or tampered ciphertext)".to_string())?;
        String::from_utf8(plaintext).map_err(|_| "decrypted value is not valid UTF-8".to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // A deterministic, clearly-fake 32-byte key (base64) for tests only.
    fn test_key() -> String {
        STANDARD.encode([7u8; 32])
    }

    #[test]
    fn roundtrip() {
        let cipher = TokenCipher::from_base64_key(&test_key()).unwrap();
        let secret = "gho_exampletoken1234567890";
        let sealed = cipher.encrypt(secret).unwrap();
        assert_ne!(
            sealed.as_slice(),
            secret.as_bytes(),
            "must not store plaintext"
        );
        assert_eq!(cipher.decrypt(&sealed).unwrap(), secret);
    }

    #[test]
    fn nonce_is_random_per_encrypt() {
        let cipher = TokenCipher::from_base64_key(&test_key()).unwrap();
        let a = cipher.encrypt("same-input").unwrap();
        let b = cipher.encrypt("same-input").unwrap();
        assert_ne!(a, b, "reused nonce would leak equality of plaintexts");
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let cipher = TokenCipher::from_base64_key(&test_key()).unwrap();
        let mut sealed = cipher.encrypt("secret").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0xff;
        assert!(cipher.decrypt(&sealed).is_err());
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let a = TokenCipher::from_base64_key(&STANDARD.encode([1u8; 32])).unwrap();
        let b = TokenCipher::from_base64_key(&STANDARD.encode([2u8; 32])).unwrap();
        let sealed = a.encrypt("secret").unwrap();
        assert!(b.decrypt(&sealed).is_err());
    }

    #[test]
    fn rejects_bad_key_length() {
        assert!(TokenCipher::from_base64_key(&STANDARD.encode([0u8; 16])).is_err());
        assert!(TokenCipher::from_base64_key("not-base64!!!").is_err());
    }

    #[test]
    fn rejects_short_blob() {
        let cipher = TokenCipher::from_base64_key(&test_key()).unwrap();
        assert!(cipher.decrypt(&[0u8; 4]).is_err());
    }
}
