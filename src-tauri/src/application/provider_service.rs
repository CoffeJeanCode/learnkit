use std::sync::Arc;

use crate::domain::provider::{ProviderConfig, ProviderKind, ProviderWithStatus};
use crate::domain::model::ModelInfo;
use crate::error::AppResult;
use crate::persistence::FileStore;
use crate::providers::{ProviderFactory, ProviderRegistry};
use crate::secrets::SecretVault;

/// Application service for BYOK providers. Owns the non-sensitive registry;
/// keys flow only between the caller and the [`SecretVault`].
pub struct ProviderService {
    registry: Arc<ProviderRegistry>,
    vault: Arc<dyn SecretVault>,
    store: FileStore,
}

impl ProviderService {
    pub fn new(registry: Arc<ProviderRegistry>, vault: Arc<dyn SecretVault>, store: FileStore) -> Self {
        Self { registry, vault, store }
    }

    pub fn list(&self) -> Vec<ProviderWithStatus> {
        let mut out: Vec<ProviderWithStatus> = self
            .registry
            .list()
            .into_iter()
            .map(|c| {
                let configured = self.vault.has_provider_key(&c.id).unwrap_or(false);
                ProviderWithStatus {
                    id: c.id,
                    provider: c.provider,
                    name: c.name,
                    default_model: c.default_model,
                    base_url: c.base_url,
                    configured,
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    #[tracing::instrument(skip(self, config), fields(provider_id = %config.id))]
    pub fn save_provider(&self, config: ProviderConfig) -> AppResult<ProviderWithStatus> {
        if config.id.trim().is_empty() {
            return Err(crate::error::AppError::InvalidInput("provider id is required".to_string()));
        }
        self.registry.upsert(config.clone());
        self.persist()?;
        Ok(ProviderWithStatus {
            configured: self.vault.has_provider_key(&config.id).unwrap_or(false),
            id: config.id,
            provider: config.provider,
            name: config.name,
            default_model: config.default_model,
            base_url: config.base_url,
        })
    }

    pub fn delete_provider(&self, id: &str) -> AppResult<()> {
        self.registry.remove(id)?;
        // Deleting a provider also drops its key: no orphan secrets.
        let _ = self.vault.delete_provider_key(id);
        self.persist()
    }

    /// Store (or replace) the API key. The key is accepted once and never returned.
    #[tracing::instrument(skip(self, api_key), fields(provider_id = %provider_id))]
    pub fn save_provider_key(&self, provider_id: &str, api_key: &str) -> AppResult<bool> {
        self.registry.get(provider_id)?;
        if api_key.trim().is_empty() {
            return Err(crate::error::AppError::InvalidInput("api key is empty".to_string()));
        }
        self.vault.save_provider_key(provider_id, api_key)?;
        Ok(true)
    }

    pub fn provider_has_key(&self, provider_id: &str) -> AppResult<bool> {
        self.registry.get(provider_id)?;
        Ok(self.vault.has_provider_key(provider_id)?)
    }

    pub fn delete_provider_key(&self, provider_id: &str) -> AppResult<()> {
        self.registry.get(provider_id)?;
        self.vault.delete_provider_key(provider_id)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(provider_id = %provider_id))]
    pub async fn test_provider(&self, provider_id: &str) -> AppResult<()> {
        let config = self.registry.get(provider_id)?;
        let key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| crate::error::AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::verify_connection(&config, &key).await
    }

    pub async fn list_models(&self, provider_id: &str) -> AppResult<Vec<ModelInfo>> {
        let config = self.registry.get(provider_id)?;
        let key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| crate::error::AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::list_models(&config, &key).await
    }

    pub fn suggested_models(&self, kind: ProviderKind) -> Vec<String> {
        kind.suggested_models().iter().map(|s| s.to_string()).collect()
    }

    fn persist(&self) -> AppResult<()> {
        self.store.save_providers(&self.registry.list())
    }
}
