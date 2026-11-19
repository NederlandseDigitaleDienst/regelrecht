//! Where the app keeps its key and the credential bound to it.
//!
//! The trait [`KeyStore`] is what the core asks: a signer for the app key,
//! made on first use, and the credential. The core never sees the private
//! key itself, only [`Es256Signer`], so a store whose key cannot leave
//! secure hardware fits behind the same trait.
//!
//! What is here is [`SoftwareKeyStore`]: the key in this process, kept in
//! memory or in a file with permissions for the user alone. That is no
//! hardware protection, and [`KeyStorage`] says so to the screens.
//!
//! # Hardware-bound storage (not built)
//!
//! A phone build implements [`KeyStore`] on the platform's key store, with
//! [`KeyStorage::Hardware`] as its kind:
//!
//! - Android: a P-256 key in the Android Keystore
//!   (`KeyPairGenerator` with `KeyProperties.KEY_ALGORITHM_EC`, purpose
//!   `SIGN`, `setIsStrongBoxBacked(true)` where available), reached through
//!   a Tauri mobile plugin in Kotlin;
//! - iOS: a key in the Secure Enclave (`kSecAttrTokenIDSecureEnclave`,
//!   `SecKeyCreateSignature` with
//!   `ecdsaSignatureMessageX962SHA256`), through a plugin in Swift.
//!
//! Both sign in DER; [`Es256Signer::sign_es256`] returns `r` and `s` of
//! 32 bytes each, so the plugin converts. The credential is no key and can
//! sit in the platform's encrypted preferences or keychain. None of this is
//! written or tested here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use regelrecht_fsc4b::jose::{Es256Signer, JoseError, SigningKey};
use serde::Serialize;

/// The file of the app key in a [`SoftwareKeyStore`] directory.
pub const KEY_FILE: &str = "app-key.pem";
/// The file of the credential in a [`SoftwareKeyStore`] directory.
pub const CREDENTIAL_FILE: &str = "credential.jwt";

/// Why the store fails.
#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("the key store at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the app key: {0}")]
    Key(#[from] JoseError),
}

/// Where the key lives, for the screens to say honestly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyStorage {
    /// In the memory of this process; gone when the app closes.
    Memory,
    /// In a file on this device, readable by the user and whoever acts as
    /// them; no hardware protects it.
    File { path: String },
    /// In secure hardware that signs without handing the key out. No store
    /// of this kind is built yet.
    Hardware { platform: String },
}

impl KeyStorage {
    /// Whether secure hardware keeps the key.
    pub fn hardware_backed(&self) -> bool {
        matches!(self, Self::Hardware { .. })
    }
}

/// Where the app keeps its key and its credential.
pub trait KeyStore: Send + Sync {
    /// Where the key lives.
    fn storage(&self) -> KeyStorage;

    /// The app key, made on the first call and the same after.
    ///
    /// # Errors
    ///
    /// When the key cannot be made, read or kept.
    fn signer(&self) -> Result<Arc<dyn Es256Signer>, KeyStoreError>;

    /// The credential kept with the key, if any.
    ///
    /// # Errors
    ///
    /// When the store cannot be read.
    fn credential(&self) -> Result<Option<String>, KeyStoreError>;

    /// Keep `credential` with the key.
    ///
    /// # Errors
    ///
    /// When the store cannot be written.
    fn store_credential(&self, credential: &str) -> Result<(), KeyStoreError>;

    /// Forget the key and the credential, so that the next activation
    /// starts with a new key.
    ///
    /// # Errors
    ///
    /// When the store cannot be cleared.
    fn reset(&self) -> Result<(), KeyStoreError>;
}

#[derive(Default)]
struct Held {
    key: Option<Arc<SigningKey>>,
    credential: Option<String>,
}

/// The key in this process, in memory or in a directory of its own.
pub struct SoftwareKeyStore {
    dir: Option<PathBuf>,
    held: Mutex<Held>,
}

impl std::fmt::Debug for SoftwareKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareKeyStore")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> KeyStoreError + '_ {
    move |source| KeyStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// `contents` in a new file at `path`, readable by the user alone, in
/// place of what was there.
fn write_private(path: &Path, contents: &str) -> Result<(), KeyStoreError> {
    let tmp = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(io_error(&tmp))?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(io_error(&tmp))?;
    fs::rename(&tmp, path).map_err(io_error(path))
}

fn read_optional(path: &Path) -> Result<Option<String>, KeyStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path)(e)),
    }
}

fn remove_optional(path: &Path) -> Result<(), KeyStoreError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_error(path)(e)),
        _ => Ok(()),
    }
}

impl SoftwareKeyStore {
    /// A store in memory: the key is gone when the process ends.
    pub fn in_memory() -> Self {
        Self {
            dir: None,
            held: Mutex::default(),
        }
    }

    /// A store in the directory `dir`, made when it does not exist.
    ///
    /// # Errors
    ///
    /// When the directory cannot be made.
    pub fn in_dir(dir: &Path) -> Result<Self, KeyStoreError> {
        fs::create_dir_all(dir).map_err(io_error(dir))?;
        Ok(Self {
            dir: Some(dir.to_path_buf()),
            held: Mutex::default(),
        })
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        // What the lock guards is plain data that no panic leaves half
        // written, so a poisoned lock is taken over as it is.
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn path(&self, file: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(file))
    }
}

impl KeyStore for SoftwareKeyStore {
    fn storage(&self) -> KeyStorage {
        match self.path(KEY_FILE) {
            None => KeyStorage::Memory,
            Some(path) => KeyStorage::File {
                path: path.display().to_string(),
            },
        }
    }

    fn signer(&self) -> Result<Arc<dyn Es256Signer>, KeyStoreError> {
        // Held until the key is kept: two first calls at once make one key.
        let mut held = self.held();
        if let Some(key) = &held.key {
            return Ok(Arc::clone(key) as Arc<dyn Es256Signer>);
        }
        let file = self.path(KEY_FILE);
        let key = match file.as_deref().map(read_optional).transpose()?.flatten() {
            Some(pem) => SigningKey::from_pkcs8_pem(&pem)?,
            None => {
                let key = SigningKey::generate()?;
                if let Some(path) = &file {
                    write_private(path, &key.to_pkcs8_pem()?)?;
                }
                key
            }
        };
        let key = Arc::new(key);
        held.key = Some(Arc::clone(&key));
        Ok(key)
    }

    fn credential(&self) -> Result<Option<String>, KeyStoreError> {
        let mut held = self.held();
        if held.credential.is_none()
            && let Some(path) = self.path(CREDENTIAL_FILE)
        {
            held.credential = read_optional(&path)?.map(|c| c.trim().to_string());
        }
        let credential = held.credential.clone();
        drop(held);
        Ok(credential)
    }

    fn store_credential(&self, credential: &str) -> Result<(), KeyStoreError> {
        if let Some(path) = self.path(CREDENTIAL_FILE) {
            write_private(&path, credential)?;
        }
        self.held().credential = Some(credential.to_string());
        Ok(())
    }

    fn reset(&self) -> Result<(), KeyStoreError> {
        // Held while the files go, so no `signer` reads a half-removed store.
        let mut held = self.held();
        for file in [KEY_FILE, CREDENTIAL_FILE] {
            if let Some(path) = self.path(file) {
                remove_optional(&path)?;
            }
        }
        *held = Held::default();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_memory_store_keeps_one_key_until_reset() {
        let store = SoftwareKeyStore::in_memory();
        assert_eq!(store.storage(), KeyStorage::Memory);
        assert!(!store.storage().hardware_backed());
        let a = store.signer().unwrap().public_jwk().thumbprint();
        assert_eq!(store.signer().unwrap().public_jwk().thumbprint(), a);
        store.store_credential("c").unwrap();
        store.reset().unwrap();
        assert_eq!(store.credential().unwrap(), None);
        assert_ne!(store.signer().unwrap().public_jwk().thumbprint(), a);
    }

    #[test]
    fn a_file_store_finds_its_key_and_credential_again() {
        let dir = tempfile::tempdir().unwrap();
        let first = SoftwareKeyStore::in_dir(dir.path()).unwrap();
        let thumbprint = first.signer().unwrap().public_jwk().thumbprint();
        first.store_credential("header.claims.signature").unwrap();
        let again = SoftwareKeyStore::in_dir(dir.path()).unwrap();
        assert_eq!(
            again.signer().unwrap().public_jwk().thumbprint(),
            thumbprint
        );
        assert_eq!(
            again.credential().unwrap().as_deref(),
            Some("header.claims.signature")
        );
        assert!(matches!(again.storage(), KeyStorage::File { .. }));
        again.reset().unwrap();
        assert!(!dir.path().join(KEY_FILE).exists());
        assert!(!dir.path().join(CREDENTIAL_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_for_the_user_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        SoftwareKeyStore::in_dir(dir.path())
            .unwrap()
            .signer()
            .unwrap();
        let mode = fs::metadata(dir.path().join(KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
}
