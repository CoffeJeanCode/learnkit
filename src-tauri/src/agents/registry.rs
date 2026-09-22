use std::collections::HashMap;
use std::sync::RwLock;

use crate::domain::agent::AgentDefinition;
use crate::error::{AppError, AppResult};
use crate::tools::is_known_tool;

/// Registry of agent definitions. Agents are materialized into Rig agents on
/// every execution (see [`ProviderFactory`](crate::providers::ProviderFactory)) —
/// there is no hardcoded agent match anywhere.
#[derive(Debug, Default)]
pub struct AgentRegistry {
    inner: RwLock<HashMap<String, AgentDefinition>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<String, AgentDefinition>>, AppError> {
        self.inner.read().map_err(|_| AppError::Persistence("agent registry lock poisoned".to_string()))
    }

    fn lock_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, AgentDefinition>>, AppError> {
        self.inner.write().map_err(|_| AppError::Persistence("agent registry lock poisoned".to_string()))
    }

    pub fn validate(def: &AgentDefinition) -> AppResult<()> {
        if def.id.trim().is_empty() {
            return Err(AppError::InvalidInput("agent id is required".to_string()));
        }
        if def.name.trim().is_empty() {
            return Err(AppError::InvalidInput("agent name is required".to_string()));
        }
        if def.system_prompt.trim().is_empty() {
            return Err(AppError::InvalidInput("system prompt is required".to_string()));
        }
        if def.model.provider_id.trim().is_empty() || def.model.model.trim().is_empty() {
            return Err(AppError::ModelNotConfigured(def.id.clone()));
        }
        for tool_id in &def.tools {
            if !is_known_tool(tool_id) {
                return Err(AppError::ToolNotFound(tool_id.clone()));
            }
        }
        Ok(())
    }

    pub fn register(&self, def: AgentDefinition) -> AppResult<()> {
        Self::validate(&def)?;
        if let Ok(mut map) = self.inner.write() {
            map.insert(def.id.clone(), def);
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> AppResult<AgentDefinition> {
        self.lock_read()?
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::AgentNotFound(id.to_string()))
    }

    pub fn list(&self) -> Vec<AgentDefinition> {
        self.lock_read().map(|m| m.values().cloned().collect()).unwrap_or_default()
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let mut map = self.lock_write()?;
        map.remove(id)
            .map(|_| ())
            .ok_or_else(|| AppError::AgentNotFound(id.to_string()))
    }

    pub fn replace_all(&self, defs: Vec<AgentDefinition>) {
        if let Ok(mut map) = self.inner.write() {
            map.clear();
            for d in defs {
                map.insert(d.id.clone(), d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::ModelRef;

    fn sample(id: &str) -> AgentDefinition {
        AgentDefinition {
            id: id.to_string(),
            name: "Test".to_string(),
            description: None,
            system_prompt: "Be helpful.".to_string(),
            model: ModelRef::new("openai", "gpt-5"),
            tools: vec![],
        }
    }

    #[test]
    fn register_get_list_remove() {
        let r = AgentRegistry::new();
        r.register(sample("a")).expect("register");
        assert_eq!(r.get("a").expect("get").name, "Test");
        assert_eq!(r.list().len(), 1);
        r.remove("a").expect("remove");
        assert!(matches!(r.get("a"), Err(AppError::AgentNotFound(_))));
    }

    #[test]
    fn rejects_unknown_tool_and_missing_model() {
        let r = AgentRegistry::new();
        let mut bad = sample("bad");
        bad.tools = vec!["shell".to_string()];
        assert!(matches!(r.register(bad), Err(AppError::ToolNotFound(_))));
        let mut bad_model = sample("bad2");
        bad_model.model = ModelRef::new("", "");
        assert!(matches!(r.register(bad_model), Err(AppError::ModelNotConfigured(_))));
    }
}
