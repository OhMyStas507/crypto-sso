//! Authenticated, atomic wallet storage. The master key is bound to the current Windows user.
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use fs2::FileExt;
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, String>;
const AAD: &[u8] = b"crypto-sso/wallet/v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

pub struct Vault {
    directory: PathBuf,
    _writer_lock: File,
    key: Option<Zeroizing<Vec<u8>>>,
}

impl Vault {
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory).map_err(|_| "STORAGE_UNAVAILABLE")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("writer.lock"))
            .map_err(|_| "STORAGE_UNAVAILABLE")?;
        lock.try_lock_exclusive()
            .map_err(|_| "WALLET_ALREADY_OPEN")?;
        Ok(Self {
            directory,
            _writer_lock: lock,
            key: None,
        })
    }
    pub fn initialized(&self) -> bool {
        self.directory.join("vault.enc").exists()
    }
    pub fn unlocked(&self) -> bool {
        self.key.is_some()
    }
    pub fn lock(&mut self) {
        self.key = None;
    }

    pub fn initialize<T: Serialize>(&mut self, document: &T) -> Result<()> {
        if self.initialized() || self.directory.join("profile.json").exists() {
            return Err("WALLET_ALREADY_INITIALIZED".into());
        }
        let key = if self.directory.join("key.dpapi").exists() {
            // Resume an initialization interrupted after key protection but before the first vault commit.
            Zeroizing::new(protect(
                &fs::read(self.directory.join("key.dpapi")).map_err(|_| "STORAGE_UNAVAILABLE")?,
                true,
            )?)
        } else {
            let mut key = Zeroizing::new(vec![0u8; 32]);
            OsRng.fill_bytes(&mut key);
            atomic_write(&self.directory.join("key.dpapi"), &protect(&key, false)?)?;
            key
        };
        self.key = Some(key);
        self.save(document)?;
        atomic_write(
            &self.directory.join("profile.json"),
            br#"{"format":"crypto-sso-wallet","version":1}"#,
        )
    }

    pub fn unlock<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        if self.directory.join("profile.json").exists() {
            let profile: serde_json::Value = serde_json::from_slice(
                &fs::read(self.directory.join("profile.json"))
                    .map_err(|_| "STORAGE_UNAVAILABLE")?,
            )
            .map_err(|_| "VAULT_CORRUPT")?;
            if profile["version"] != 1 {
                return Err("UNSUPPORTED_VAULT_VERSION".into());
            }
        }
        let protected =
            fs::read(self.directory.join("key.dpapi")).map_err(|_| "STORAGE_UNAVAILABLE")?;
        let key = Zeroizing::new(protect(&protected, true)?);
        if key.len() != 32 {
            return Err("VAULT_CORRUPT".into());
        }
        self.key = Some(key);
        match self.load() {
            Ok(value) => Ok(value),
            Err(error) => {
                self.lock();
                Err(error)
            }
        }
    }

    pub fn load<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        let key = self.key.as_ref().ok_or("WALLET_LOCKED")?;
        let envelope: Envelope = serde_json::from_slice(
            &fs::read(self.directory.join("vault.enc")).map_err(|_| "STORAGE_UNAVAILABLE")?,
        )
        .map_err(|_| "VAULT_CORRUPT")?;
        if envelope.version != 1 {
            return Err("UNSUPPORTED_VAULT_VERSION".into());
        }
        let nonce = hex::decode(&envelope.nonce).map_err(|_| "VAULT_CORRUPT")?;
        if nonce.len() != 12 {
            return Err("VAULT_CORRUPT".into());
        }
        let ciphertext = hex::decode(envelope.ciphertext).map_err(|_| "VAULT_CORRUPT")?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "VAULT_CORRUPT")?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    nonce.as_slice().into(),
                    Payload {
                        msg: &ciphertext,
                        aad: AAD,
                    },
                )
                .map_err(|_| "VAULT_CORRUPT")?,
        );
        serde_json::from_slice(&plain).map_err(|_| "VAULT_CORRUPT".into())
    }

    pub fn save<T: Serialize>(&self, document: &T) -> Result<()> {
        let key = self.key.as_ref().ok_or("WALLET_LOCKED")?;
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "VAULT_CORRUPT")?;
        let plain =
            Zeroizing::new(serde_json::to_vec(document).map_err(|_| "STORAGE_UNAVAILABLE")?);
        let ciphertext = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plain,
                    aad: AAD,
                },
            )
            .map_err(|_| "STORAGE_UNAVAILABLE")?;
        let data = serde_json::to_vec(&Envelope {
            version: 1,
            nonce: hex::encode(nonce),
            ciphertext: hex::encode(ciphertext),
        })
        .map_err(|_| "STORAGE_UNAVAILABLE")?;
        atomic_write(&self.directory.join("vault.enc"), &data)?;
        // Verify the committed file before reporting a usable credential.
        let _: serde_json::Value = self.load()?;
        Ok(())
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().ok_or("STORAGE_UNAVAILABLE")?)
        .map_err(|_| "STORAGE_UNAVAILABLE")?;
    tmp.write_all(bytes)
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|_| "STORAGE_UNAVAILABLE")?;
    tmp.persist(path).map_err(|_| "STORAGE_UNAVAILABLE")?;
    #[cfg(unix)]
    File::open(path.parent().unwrap())
        .and_then(|file| file.sync_all())
        .map_err(|_| "STORAGE_UNAVAILABLE")?;
    Ok(())
}

#[cfg(windows)]
fn protect(input: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let source = CRYPT_INTEGER_BLOB {
        cbData: input
            .len()
            .try_into()
            .map_err(|_| "KEY_PROTECTION_FAILED")?,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut destination = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI owns destination. Copy it, then free the original with the corresponding Windows allocator.
    let success = unsafe {
        if decrypt {
            CryptUnprotectData(
                &source,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut destination,
            )
        } else {
            CryptProtectData(
                &source,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut destination,
            )
        }
    };
    if success == 0 {
        return Err("KEY_PROTECTION_FAILED".into());
    }
    let result = unsafe {
        std::slice::from_raw_parts(destination.pbData, destination.cbData as usize).to_vec()
    };
    unsafe {
        std::ptr::write_bytes(destination.pbData, 0, destination.cbData as usize);
        LocalFree(destination.pbData.cast());
    }
    Ok(result)
}

#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>> {
    Err("OS_KEY_PROTECTION_UNSUPPORTED".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn encrypted_atomic_reopen_tamper_and_single_writer() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Vault::open(dir.path()).unwrap();
        assert!(Vault::open(dir.path()).is_err());
        let data = serde_json::json!({"secret":"unique-pending-secret", "claims":{"name":"Alice"}});
        vault.initialize(&data).unwrap();
        let bytes = fs::read(dir.path().join("vault.enc")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("unique-pending-secret"));
        vault.lock();
        assert_eq!(vault.unlock::<serde_json::Value>().unwrap(), data);
        let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
        envelope.ciphertext.replace_range(0..2, "ff");
        fs::write(
            dir.path().join("vault.enc"),
            serde_json::to_vec(&envelope).unwrap(),
        )
        .unwrap();
        vault.lock();
        assert!(vault.unlock::<serde_json::Value>().is_err());
        assert!(!vault.unlocked());
    }
}
