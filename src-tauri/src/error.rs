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
    #[error("provider connection failed")]
    ProviderConnectionFailed(String),
    #[error("agent execution failed")]
    AgentExecutionFailed(String),
    #[error("secret vault error")]
    Vault(String),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
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
}
