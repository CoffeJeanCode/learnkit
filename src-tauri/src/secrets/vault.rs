use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, RwLock};

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault backend failure")]
    Backend(String),
}

impl From<VaultError> for AppError {
    fn from(e: VaultError) -> Self {
        // Never include key material; Backend carries only a static label.
        AppError::Vault(e.to_string())
    }
}

/// Key storage abstraction. Implementations must never log or return key material.
pub trait SecretVault: Send + Sync {
    fn save_provider_key(&self, provider_id: &str, key: &str) -> Result<(), VaultError>;
    /// Batch variant so snapshot-backed vaults persist once instead of once
    /// per key — every persist re-encrypts the whole snapshot (seconds in
    /// debug builds, hence startup must not call it in a loop). The default
    /// impl just loops for vaults where persist is cheap.
    fn save_provider_keys(&self, keys: &[(String, String)]) -> Result<(), VaultError> {
        for (provider_id, key) in keys {
            self.save_provider_key(provider_id, key)?;
        }
        Ok(())
    }
    fn get_provider_key(&self, provider_id: &str) -> Result<Option<String>, VaultError>;
    fn delete_provider_key(&self, provider_id: &str) -> Result<(), VaultError>;
    fn has_provider_key(&self, provider_id: &str) -> Result<bool, VaultError> {
        Ok(self.get_provider_key(provider_id)?.is_some())
    }
}

fn storage_key(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

// ---------------------------------------------------------------------------
// In-memory vault (tests, ephemeral fallback)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MemoryVault {
    map: RwLock<HashMap<String, String>>,
}

impl MemoryVault {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretVault for MemoryVault {
    fn save_provider_key(&self, provider_id: &str, key: &str) -> Result<(), VaultError> {
        self.map
            .write()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?
            .insert(storage_key(provider_id), key.to_string());
        Ok(())
    }

    fn get_provider_key(&self, provider_id: &str) -> Result<Option<String>, VaultError> {
        Ok(self
            .map
            .read()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?
            .get(&storage_key(provider_id))
            .cloned())
    }

    fn delete_provider_key(&self, provider_id: &str) -> Result<(), VaultError> {
        self.map
            .write()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?
            .remove(&storage_key(provider_id));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stronghold vault (production)
// ---------------------------------------------------------------------------

/// Resolve the snapshot password: env `LEARNKIT_VAULT_PASSWORD` first, else a
/// documented dev default. Production hardening: prompt the user or use the OS
/// keychain and pass the secret here. The password is hashed to 32 bytes
/// (Stronghold requirement) and zeroized after use.
pub fn resolve_vault_password() -> Zeroizing<Vec<u8>> {
    match std::env::var("LEARNKIT_VAULT_PASSWORD") {
        Ok(pw) if !pw.trim().is_empty() => {
            let mut hasher = Sha256::new();
            hasher.update(pw.as_bytes());
            Zeroizing::new(hasher.finalize().to_vec())
        }
        _ => {
            tracing::warn!(
                "LEARNKIT_VAULT_PASSWORD not set; using dev-only vault password. Set the env var for real usage."
            );
            let mut hasher = Sha256::new();
            hasher.update(b"learnkit-dev-vault-password::change-me");
            Zeroizing::new(hasher.finalize().to_vec())
        }
    }
}

const STRONGHOLD_CLIENT: &[u8] = b"learnkit";

/// Production vault backed by the IOTA Stronghold engine (same engine as
/// `tauri-plugin-stronghold`), persisted encrypted at `<data_dir>/vault.hold`.
pub struct StrongholdVault {
    inner: Mutex<tauri_plugin_stronghold::stronghold::Stronghold>,
}

impl StrongholdVault {
    pub fn open(data_dir: &Path, password: &[u8]) -> Result<Self, VaultError> {
        let snapshot = data_dir.join("vault.hold");
        let stronghold = tauri_plugin_stronghold::stronghold::Stronghold::new(
            snapshot,
            password.to_vec(),
        )
        .map_err(|e| VaultError::Backend(format!("open snapshot: {e}")))?;
        let vault = Self {
            inner: Mutex::new(stronghold),
        };
        vault.ensure_client()?;
        #[cfg(debug_assertions)]
        if fast_dev_kdf_active() {
            // One cheap save re-encrypts with the fast work factor, so a
            // vault.hold written with full strength migrates itself on first
            // open instead of paying full decrypt cost on every startup.
            let inner =
                vault.inner.lock().map_err(|_| VaultError::Backend("lock poisoned".to_string()))?;
            Self::persist(&inner)?;
        }
        Ok(vault)
    }

    fn ensure_client(&self) -> Result<(), VaultError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?;
        // get -> load (after restart) -> create (first run)
        if inner.get_client(STRONGHOLD_CLIENT).is_err() {
            if inner.load_client(STRONGHOLD_CLIENT).is_err() {
                inner
                    .create_client(STRONGHOLD_CLIENT)
                    .map_err(|e| VaultError::Backend(format!("create client: {e}")))?;
                inner
                    .save()
                    .map_err(|e| VaultError::Backend(format!("persist snapshot: {e}")))?;
            }
        }
        Ok(())
    }

    fn persist(inner: &tauri_plugin_stronghold::stronghold::Stronghold) -> Result<(), VaultError> {
        inner
            .save()
            .map_err(|e| VaultError::Backend(format!("persist snapshot: {e}")))
    }
}

impl SecretVault for StrongholdVault {
    fn save_provider_key(&self, provider_id: &str, key: &str) -> Result<(), VaultError> {
        self.save_provider_keys(&[(provider_id.to_string(), key.to_string())])
    }

    fn save_provider_keys(&self, keys: &[(String, String)]) -> Result<(), VaultError> {
        if keys.is_empty() {
            return Ok(());
        }
        let inner = self
            .inner
            .lock()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?;
        let client = inner
            .get_client(STRONGHOLD_CLIENT)
            .map_err(|e| VaultError::Backend(format!("get client: {e}")))?;
        for (provider_id, key) in keys {
            client
                .store()
                .insert(
                    storage_key(provider_id).into_bytes(),
                    key.as_bytes().to_vec(),
                    None,
                )
                .map_err(|e| VaultError::Backend(format!("insert record: {e}")))?;
        }
        Self::persist(&inner)
    }

    fn get_provider_key(&self, provider_id: &str) -> Result<Option<String>, VaultError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?;
        let client = inner
            .get_client(STRONGHOLD_CLIENT)
            .map_err(|e| VaultError::Backend(format!("get client: {e}")))?;
        let raw = client
            .store()
            .get(storage_key(provider_id).as_bytes())
            .map_err(|e| VaultError::Backend(format!("read record: {e}")))?;
        match raw {
            Some(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| VaultError::Backend("stored key is not valid utf-8".to_string())),
            None => Ok(None),
        }
    }

    fn delete_provider_key(&self, provider_id: &str) -> Result<(), VaultError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| VaultError::Backend("lock poisoned".to_string()))?;
        let client = inner
            .get_client(STRONGHOLD_CLIENT)
            .map_err(|e| VaultError::Backend(format!("get client: {e}")))?;
        client
            .store()
            .delete(storage_key(provider_id).as_bytes())
            .map_err(|e| VaultError::Backend(format!("delete record: {e}")))?;
        Self::persist(&inner)
    }
}

/// Debug builds only: Stronghold's default snapshot KDF costs minutes per
/// open/save without optimizations (measured ~155s fresh open in debug vs
/// ~1s in release), freezing the window past the OS "not responding"
/// threshold. Use the test-grade work factor so dev stays usable; release
/// builds always use full strength. Set `LEARNKIT_STRONG_VAULT=1` to opt back
/// into production-strength encryption in dev (slow startup). A snapshot
/// written with full strength pays full cost to *decrypt* once, then
/// `StrongholdVault::open` re-persists it with the fast factor, so the very
/// first post-change startup may still be slow — after that it stays fast.
#[cfg(debug_assertions)]
fn maybe_use_fast_dev_kdf() {
    if std::env::var("LEARNKIT_STRONG_VAULT").as_deref() == Ok("1") {
        return;
    }
    if iota_stronghold::engine::snapshot::try_set_encrypt_work_factor(5).is_ok() {
        tracing::warn!(
            "fast dev vault: reduced snapshot KDF work factor (debug only). \
             Set LEARNKIT_STRONG_VAULT=1 for production-strength encryption in dev."
        );
    }
}

#[cfg(not(debug_assertions))]
fn maybe_use_fast_dev_kdf() {}

/// Whether the debug-only reduced snapshot KDF is in effect.
#[cfg(debug_assertions)]
fn fast_dev_kdf_active() -> bool {
    std::env::var("LEARNKIT_STRONG_VAULT").as_deref() != Ok("1")
}

/// Convenience: create a vault, falling back to memory if Stronghold cannot open.
pub fn open_vault_or_memory(data_dir: &Path) -> (Box<dyn SecretVault>, bool) {
    maybe_use_fast_dev_kdf();
    let password = resolve_vault_password();
    match StrongholdVault::open(data_dir, &password) {
        Ok(v) => (Box::new(v), true),
        Err(e) => {
            tracing::warn!("Stronghold unavailable ({e}); using ephemeral in-memory vault");
            (Box::new(MemoryVault::new()), false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_vault_roundtrip() {
        let v = MemoryVault::new();
        assert!(!v.has_provider_key("openai").expect("has"));
        v.save_provider_key("openai", "sk-test").expect("save");
        assert!(v.has_provider_key("openai").expect("has"));
        assert_eq!(v.get_provider_key("openai").expect("get").as_deref(), Some("sk-test"));
        v.delete_provider_key("openai").expect("delete");
        assert!(!v.has_provider_key("openai").expect("has"));
    }

    #[test]
    fn stronghold_vault_roundtrip_in_tempdir() {
        // TEST-ONLY: snapshot encryption uses a deliberately slow KDF
        // (~1s per commit in release, much slower in unoptimized debug).
        // Lower the work factor so the suite stays fast. NEVER do this in
        // production code — the factory default is a security property.
        let _ = iota_stronghold::engine::snapshot::try_set_encrypt_work_factor(5);

        let dir = std::env::temp_dir().join(format!("learnkit-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let mut hasher = Sha256::new();
        hasher.update(b"test-password");
        let pw = hasher.finalize().to_vec();
        let v = StrongholdVault::open(&dir, &pw).expect("open");
        v.save_provider_key("openai", "sk-test-123").expect("save");
        assert!(v.has_provider_key("openai").expect("has"));
        assert_eq!(
            v.get_provider_key("openai").expect("get").as_deref(),
            Some("sk-test-123")
        );
        // Reopen from the same snapshot: key must persist.
        drop(v);
        let v2 = StrongholdVault::open(&dir, &pw).expect("reopen");
        assert_eq!(
            v2.get_provider_key("openai").expect("get").as_deref(),
            Some("sk-test-123")
        );
        v2.delete_provider_key("openai").expect("delete");
        assert!(!v2.has_provider_key("openai").expect("has"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
