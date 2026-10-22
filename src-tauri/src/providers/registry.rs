use std::collections::HashMap;
use std::sync::RwLock;

use crate::domain::provider::{ProviderConfig, ProviderKind};
use crate::error::{AppError, AppResult};

/// Registry of non-sensitive provider configurations (BYOK).
/// API keys are never stored here — see [`SecretVault`](crate::secrets::SecretVault).
#[derive(Debug, Default)]
pub struct ProviderRegistry {
    inner: RwLock<HashMap<String, ProviderConfig>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the built-in provider slots (without keys or models).
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        for kind in ProviderKind::all() {
            let id = registry.default_id(kind);
            registry.upsert(ProviderConfig {
                id: id.clone(),
                provider: *kind,
                name: kind.display_name().to_string(),
                default_model: None,
                base_url: None,
            });
        }
        registry
    }

    fn default_id(&self, kind: &ProviderKind) -> String {
        match kind {
            ProviderKind::OpenAI => "openai".to_string(),
            ProviderKind::Anthropic => "anthropic".to_string(),
            ProviderKind::Gemini => "gemini".to_string(),
            ProviderKind::OpenRouter => "openrouter".to_string(),
            ProviderKind::DeepSeek => "deepseek".to_string(),
            ProviderKind::Xiaomi => "xiaomi".to_string(),
        }
    }

    fn lock_read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, ProviderConfig>>, AppError> {
        self.inner.read().map_err(|_| AppError::Persistence("provider registry lock poisoned".to_string()))
    }

    fn lock_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, ProviderConfig>>, AppError> {
        self.inner.write().map_err(|_| AppError::Persistence("provider registry lock poisoned".to_string()))
    }

    pub fn upsert(&self, config: ProviderConfig) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(config.id.clone(), config);
        }
    }

    /// Insert default slots for built-in providers missing from the
    /// registry, without touching existing entries. Covers upgrades: a
    /// `providers.json` persisted before a new provider shipped would
    /// otherwise shadow it forever via `replace_all`.
    pub fn ensure_defaults(&self) {
        for kind in ProviderKind::all() {
            let id = self.default_id(kind);
            if self.get(&id).is_err() {
                self.upsert(ProviderConfig {
                    id,
                    provider: *kind,
                    name: kind.display_name().to_string(),
                    default_model: None,
                    base_url: None,
                });
            }
        }
    }

    pub fn get(&self, id: &str) -> AppResult<ProviderConfig> {
        self.lock_read()?
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotConfigured(id.to_string()))
    }

    pub fn list(&self) -> Vec<ProviderConfig> {
        self.lock_read().map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let mut map = self.lock_write()?;
        map.remove(id)
            .map(|_| ())
            .ok_or_else(|| AppError::ProviderNotConfigured(id.to_string()))
    }

    pub fn replace_all(&self, configs: Vec<ProviderConfig>) {
        if let Ok(mut map) = self.inner.write() {
            map.clear();
            for c in configs {
                map.insert(c.id.clone(), c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProviderConfig {
        ProviderConfig {
            id: "openai".to_string(),
            provider: ProviderKind::OpenAI,
            name: "OpenAI".to_string(),
            default_model: Some("gpt-5".to_string()),
            base_url: None,
        }
    }

    #[test]
    fn upsert_get_list_remove() {
        let r = ProviderRegistry::new();
        r.upsert(sample());
        assert_eq!(r.get("openai").expect("get").name, "OpenAI");
        assert_eq!(r.list().len(), 1);
        r.remove("openai").expect("remove");
        assert!(matches!(r.get("openai"), Err(AppError::ProviderNotConfigured(_))));
    }

    #[test]
    fn defaults_seed_all_providers() {
        let r = ProviderRegistry::with_defaults();
        assert_eq!(r.list().len(), 6);
    }

    #[test]
    fn ensure_defaults_backfills_new_builtin_without_touching_existing() {
        // Simulates a `providers.json` persisted before Xiaomi shipped.
        let r = ProviderRegistry::new();
        r.upsert(ProviderConfig {
            id: "openai".to_string(),
            provider: ProviderKind::OpenAI,
            name: "OpenAI".to_string(),
            default_model: Some("gpt-5".to_string()),
            base_url: Some("https://custom.example.com".to_string()),
        });
        r.ensure_defaults();
        assert_eq!(r.list().len(), 6);
        // Existing customizations survive the backfill.
        let openai = r.get("openai").expect("openai kept");
        assert_eq!(openai.default_model.as_deref(), Some("gpt-5"));
        assert_eq!(openai.base_url.as_deref(), Some("https://custom.example.com"));
        assert_eq!(r.get("xiaomi").expect("xiaomi backfilled").name, "Xiaomi MiMo");
    }
}
