use std::sync::Arc;

use crate::domain::agent::{AgentDefinition, AgentOutput};
use crate::error::AppResult;
use crate::agents::AgentRegistry;
use crate::orchestration::Orchestrator;
use crate::persistence::FileStore;
use crate::tools::ToolInfo;

/// Application service for agents: CRUD over definitions + execution via the orchestrator.
pub struct AgentService {
    registry: Arc<AgentRegistry>,
    orchestrator: Arc<Orchestrator>,
    store: FileStore,
}

impl AgentService {
    pub fn new(registry: Arc<AgentRegistry>, orchestrator: Arc<Orchestrator>, store: FileStore) -> Self {
        Self { registry, orchestrator, store }
    }

    pub fn list(&self) -> Vec<AgentDefinition> {
        let mut out = self.registry.list();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn get(&self, id: &str) -> AppResult<AgentDefinition> {
        self.registry.get(id)
    }

    #[tracing::instrument(skip(self, def), fields(agent_id = %def.id))]
    pub fn create(&self, def: AgentDefinition) -> AppResult<AgentDefinition> {
        self.registry.register(def.clone())?;
        self.persist()?;
        Ok(def)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        self.registry.remove(id)?;
        self.persist()
    }

    #[tracing::instrument(skip(self, input), fields(agent_id = %agent_id))]
    pub async fn run(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
    ) -> AppResult<AgentOutput> {
        self.orchestrator.run_agent(app, agent_id, input, None).await
    }

    pub fn tools(&self) -> Vec<ToolInfo> {
        crate::tools::list_tools()
    }

    /// Persists the registry's current contents. Exposed (not just called
    /// internally by `create`/`delete`) so callers that mutate the registry
    /// directly — e.g. [`AppState::heal_roadmap_agent_model`](crate::state::AppState::heal_roadmap_agent_model) —
    /// can save the result without duplicating `FileStore` wiring.
    pub fn persist(&self) -> AppResult<()> {
        self.store.save_agents(&self.registry.list())
    }
}
