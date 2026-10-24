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

    /// A single tool call starting inside an agent run (emitted from the
    /// tool implementations themselves, so the UI can tell "model is
    /// thinking" from "model is running tools"). No run/agent ids: by the
    /// time a tool executes it's deep inside Rig's own loop, far from the
    /// orchestrator's `AgentContext`.
    pub fn tool(tool: &str) -> Self {
        Self {
            event: "tool".to_string(),
            run_id: String::new(),
            agent_id: String::new(),
            parent_run_id: None,
            text: None,
            tool: Some(tool.to_string()),
            error: None,
        }
    }
}

/// Per-BLOCK progress events for the iterative notebook engine — distinct
/// from `AgentEvent` (which is per AGENT RUN, e.g. "the whole
/// `notebook_generator` turn started/completed"). One of these fires for
/// every block `notebook_service::generation` buffers in the background, so
/// the frontend can reveal a newly-ready block without polling. Emitted
/// directly via the `Option<&AppHandle>` every service method already
/// receives — unlike `tools::emit_tool_event`, no `OnceLock` indirection is
/// needed here since these are emitted from application-service code, not
/// from deep inside Rig's tool-calling loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookBlockEvent {
    pub event: String,
    pub document_id: String,
    /// Present only for `event == "ready"` — already answer-key-redacted
    /// (see `notebook_service::grading::redact_block`), safe to send as-is.
    pub block: Option<crate::domain::notebook::NotebookBlock>,
    /// Present only for `event == "error"`.
    pub message: Option<String>,
    pub retriable: Option<bool>,
    /// Only for `event == "delta"`: which generator attempt this progress
    /// belongs to (1-based). A higher number than the one on screen means the
    /// previous draft was rejected and a new one started: the UI resets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u8>,
    /// Only for `event == "delta"`: the SANITIZED partial block
    /// (`tools::block_stream::preview_from_partial`) — block type plus a
    /// whitelist of reading-only prose, never raw tool arguments or answer keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<serde_json::Value>,
    /// Only for `event == "delta"`: characters the model has reasoned so far
    /// (an amount, never the text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_chars: Option<usize>,
}

impl NotebookBlockEvent {
    fn bare(event: &str, document_id: &str) -> Self {
        Self {
            event: event.to_string(),
            document_id: document_id.to_string(),
            block: None,
            message: None,
            retriable: None,
            attempt: None,
            preview: None,
            reasoning_chars: None,
        }
    }

    pub fn generating(document_id: &str) -> Self {
        Self::bare("generating", document_id)
    }

    pub fn ready(document_id: &str, block: crate::domain::notebook::NotebookBlock) -> Self {
        Self { block: Some(block), ..Self::bare("ready", document_id) }
    }

    pub fn error(document_id: &str, message: &str) -> Self {
        Self { message: Some(message.to_string()), retriable: Some(true), ..Self::bare("error", document_id) }
    }

    /// Live progress while a block is still being written.
    pub fn delta(document_id: &str, attempt: u8, preview: Option<serde_json::Value>, reasoning_chars: Option<usize>) -> Self {
        Self { attempt: Some(attempt), preview, reasoning_chars, ..Self::bare("delta", document_id) }
    }
}
