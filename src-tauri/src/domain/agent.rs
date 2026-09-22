use serde::{Deserialize, Serialize};

use super::model::ModelRef;

/// Agent definition: prompt + model + tools. Stored in [`AgentRegistry`](crate::agents::AgentRegistry)
/// and materialized into a Rig agent on every execution (no hardcoded match).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: String,
    pub model: ModelRef,
    pub tools: Vec<String>,
}

/// Execution context. Every run gets a `run_id`; delegations link back via `parent_run_id`
/// so tracing can be added later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub run_id: String,
    pub agent_id: String,
    pub parent_run_id: Option<String>,
}

/// Result of a single agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub run_id: String,
    pub agent_id: String,
    pub parent_run_id: Option<String>,
    pub text: String,
    pub tool_calls: Vec<String>,
    pub duration_ms: u64,
}
