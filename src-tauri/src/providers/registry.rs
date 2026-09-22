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
    fn defaults_seed_five_providers() {
        let r = ProviderRegistry::with_defaults();
        assert_eq!(r.list().len(), 5);
    }
}
