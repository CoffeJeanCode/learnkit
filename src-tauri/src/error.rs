use serde::Serialize;

/// Typed application errors. Serialized to `{ code, message }` for the
/// frontend. Secrets (API keys, vault passwords) must never be interpolated
/// into these messages.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("provider not configured: {0}")]
    ProviderNotConfigured(String),
    #[error("provider key missing for provider: {0}")]
    ProviderKeyMissing(String),
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("model not configured for agent: {0}")]
    ModelNotConfigured(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("provider connection failed: {0}")]
    ProviderConnectionFailed(String),
    #[error("agent execution failed: {0}")]
    AgentExecutionFailed(String),
    #[error("secret vault error")]
    Vault(String),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("operation timed out: {0}")]
    Timeout(String),
}

impl AppError {
    /// Whether retrying the same operation could plausibly succeed: model
    /// runtime failures (including tool-call errors surfaced by Rig),
    /// network blips and timeouts. Config errors (missing key, unknown
    /// agent/tool, bad input, vault/persistence failures) are permanent —
    /// retrying them would just burn another slow LLM call.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::AgentExecutionFailed(_) | Self::ProviderConnectionFailed(_) | Self::Timeout(_)
        )
    }

    /// Whether this failure is specifically the model running out of output
    /// tokens mid-response (`finish_reason=Length`, surfaced by `rig_agent`/
    /// `rig_core`) — distinct from a generic transient failure because a
    /// blind backoff-and-resend-the-same-input retry (see `is_transient`)
    /// will very likely fail identically again: the input didn't get
    /// shorter. Callers that see `true` here should inject a corrective
    /// "be more concise" note into the NEXT attempt's input instead of just
    /// waiting and resending (see `notebook_service::generation::
    /// generate_and_persist_block` and `notebook_service::grading::
    /// grade_with_model`).
    pub fn is_output_budget_exhausted(&self) -> bool {
        matches!(self, Self::AgentExecutionFailed(msg) if msg.contains("finish_reason=Length"))
    }

    pub fn code(&self) -> &'static str {        match self {
            Self::ProviderNotConfigured(_) => "ProviderNotConfigured",
            Self::ProviderKeyMissing(_) => "ProviderKeyMissing",
            Self::AgentNotFound(_) => "AgentNotFound",
            Self::ModelNotConfigured(_) => "ModelNotConfigured",
            Self::ToolNotFound(_) => "ToolNotFound",
            Self::ProviderConnectionFailed(_) => "ProviderConnectionFailed",
            Self::AgentExecutionFailed(_) => "AgentExecutionFailed",
            Self::Vault(_) => "VaultError",
            Self::Persistence(_) => "PersistenceError",
            Self::InvalidInput(_) => "InvalidInput",
            Self::Timeout(_) => "Timeout",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        // rusqlite errors never carry secrets — safe to surface verbatim.
        Self::Persistence(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_without_leaking_details() {
        let err = AppError::ProviderKeyMissing("openai".to_string());
        let v = serde_json::to_value(&err).expect("serializable");
        assert_eq!(v["code"], "ProviderKeyMissing");
        assert!(v["message"].as_str().unwrap().contains("openai"));
    }

    #[test]
    fn transient_covers_runtime_failures_only() {
        assert!(AppError::AgentExecutionFailed("tool call blew up".to_string()).is_transient());
        assert!(AppError::ProviderConnectionFailed("reset".to_string()).is_transient());
        assert!(AppError::Timeout("slow".to_string()).is_transient());
        assert!(!AppError::ProviderKeyMissing("openai".to_string()).is_transient());
        assert!(!AppError::ToolNotFound("echo".to_string()).is_transient());
        assert!(!AppError::AgentNotFound("ghost".to_string()).is_transient());
        assert!(!AppError::InvalidInput("bad".to_string()).is_transient());
    }

    #[test]
    fn output_budget_exhausted_only_matches_the_length_finish_reason() {
        assert!(
            AppError::AgentExecutionFailed(
                "CompletionError: ResponseError: the model produced no answer and stopped with finish_reason=Length".to_string()
            )
            .is_output_budget_exhausted()
        );
        assert!(!AppError::AgentExecutionFailed("some other failure".to_string()).is_output_budget_exhausted());
        assert!(!AppError::Timeout("slow".to_string()).is_output_budget_exhausted());
    }
}
