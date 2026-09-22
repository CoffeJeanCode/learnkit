use std::path::PathBuf;

use crate::domain::agent::AgentDefinition;
use crate::domain::provider::ProviderConfig;
use crate::domain::roadmap::RoadmapSession;
use crate::error::{AppError, AppResult};

/// Persists ONLY non-sensitive configuration (providers without keys, agent
/// definitions) as local JSON. Secrets always go to Stronghold.
#[derive(Debug, Clone)]
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn providers_path(&self) -> PathBuf {
        self.dir.join("providers.json")
    }

    fn agents_path(&self) -> PathBuf {
        self.dir.join("agents.json")
    }

    pub fn save_providers(&self, configs: &[ProviderConfig]) -> AppResult<()> {
        Self::write_json(&self.providers_path(), configs)
    }

    pub fn load_providers(&self) -> AppResult<Vec<ProviderConfig>> {
        Self::read_json(&self.providers_path())
    }

    pub fn save_agents(&self, defs: &[AgentDefinition]) -> AppResult<()> {
        Self::write_json(&self.agents_path(), defs)
    }

    pub fn load_agents(&self) -> AppResult<Vec<AgentDefinition>> {
        Self::read_json(&self.agents_path())
    }

    /// One file per session (unlike `agents`/`providers`, sessions grow
    /// unboundedly, so they are not kept as a single array on disk).
    fn roadmap_session_path(&self, session_id: &str) -> AppResult<PathBuf> {
        if session_id.is_empty()
            || session_id.contains(['/', '\\'])
            || session_id.contains("..")
        {
            return Err(AppError::InvalidInput(format!("invalid session id: {session_id}")));
        }
        Ok(self.dir.join("roadmap_sessions").join(format!("{session_id}.json")))
    }

    pub fn save_roadmap_session(&self, session: &RoadmapSession) -> AppResult<()> {
        let path = self.roadmap_session_path(&session.session_id)?;
        Self::write_json(&path, session)
    }

    pub fn load_roadmap_session(&self, session_id: &str) -> AppResult<Option<RoadmapSession>> {
        let path = self.roadmap_session_path(session_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AppError::Persistence(format!("read: {e}")))?;
        let session = serde_json::from_str(&raw).map_err(|e| AppError::Persistence(format!("parse: {e}")))?;
        Ok(Some(session))
    }

    fn write_json<T: serde::Serialize + ?Sized>(path: &PathBuf, value: &T) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Persistence(format!("create dir: {e}")))?;
        }
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| AppError::Persistence(format!("serialize: {e}")))?;
        std::fs::write(path, json).map_err(|e| AppError::Persistence(format!("write: {e}")))
    }

    fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> AppResult<Vec<T>> {
        if !path.exists() {
            return Ok(vec![]);
        }
        let raw = std::fs::read_to_string(path).map_err(|e| AppError::Persistence(format!("read: {e}")))?;
        serde_json::from_str(&raw).map_err(|e| AppError::Persistence(format!("parse: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::ModelRef;
    use crate::domain::provider::ProviderKind;

    #[test]
    fn persists_and_loads_without_secrets() {
        let dir = std::env::temp_dir().join(format!("learnkit-store-{}", uuid::Uuid::new_v4()));
        let store = FileStore::new(dir.clone());
        store
            .save_providers(&[ProviderConfig {
                id: "openai".to_string(),
                provider: ProviderKind::OpenAI,
                name: "OpenAI".to_string(),
                default_model: Some("gpt-5".to_string()),
                base_url: None,
            }])
            .expect("save");
        let loaded = store.load_providers().expect("load");
        assert_eq!(loaded.len(), 1);
        let raw = std::fs::read_to_string(dir.join("providers.json")).expect("raw");
        assert!(!raw.contains("sk-"), "persisted config must never contain keys");

        store
            .save_agents(&[AgentDefinition {
                id: "a".to_string(),
                name: "A".to_string(),
                description: None,
                system_prompt: "sys".to_string(),
                model: ModelRef::new("openai", "gpt-5"),
                tools: vec![],
            }])
            .expect("save agents");
        assert_eq!(store.load_agents().expect("load").len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
