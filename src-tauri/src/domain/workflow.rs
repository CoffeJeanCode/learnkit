use serde::{Deserialize, Serialize};

/// Minimal workflow types. The engine is intentionally NOT implemented yet;
/// `delegate` on the orchestrator covers the MVP. These types reserve the
/// shape for sequential / parallel / supervisor patterns later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowStep {
    RunAgent { agent_id: String, input: String },
    Delegate { from_agent: String, to_agent: String, task: String },
}
