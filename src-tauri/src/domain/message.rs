use serde::{Deserialize, Serialize};

/// A single chat turn. History is kept in frontend memory only (no persistence yet).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Events emitted to the frontend during agent execution.
/// Prepared for token streaming; the MVP emits started/completed/error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event: String,
    pub run_id: String,
    pub agent_id: String,
    pub parent_run_id: Option<String>,
    pub text: Option<String>,
    pub tool: Option<String>,
    pub error: Option<String>,
}

impl AgentEvent {
    pub fn started(run_id: &str, agent_id: &str, parent_run_id: Option<&str>) -> Self {
        Self {
            event: "started".to_string(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            parent_run_id: parent_run_id.map(str::to_string),
            text: None,
            tool: None,
            error: None,
        }
    }

    pub fn completed(run_id: &str, agent_id: &str, text: &str) -> Self {
        Self {
            event: "completed".to_string(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            parent_run_id: None,
            text: Some(text.to_string()),
            tool: None,
            error: None,
        }
    }

    pub fn failed(run_id: &str, agent_id: &str, error: &str) -> Self {
        Self {
            event: "error".to_string(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            parent_run_id: None,
            text: None,
            tool: None,
            error: Some(error.to_string()),
        }
    }
}
