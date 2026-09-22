use serde::{Deserialize, Serialize};

/// Canonical model reference: `provider_id` + model id (e.g. `openai` + `gpt-5`).
/// Displayed canonically as `provider_id/model`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: String,
    pub model: String,
}

impl ModelRef {
    pub fn new(provider_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model: model.into(),
        }
    }

    pub fn canonical(&self) -> String {
        format!("{}/{}", self.provider_id, self.model)
    }

    /// Parse `provider/model`. Everything after the first `/` is the model id,
    /// so `openrouter/anthropic/claude` parses as provider `openrouter`.
    pub fn parse_canonical(s: &str) -> Option<Self> {
        let (provider_id, model) = s.split_once('/')?;
        if provider_id.is_empty() || model.is_empty() {
            return None;
        }
        Some(Self::new(provider_id, model))
    }
}

/// Model metadata returned by provider discovery endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_refs() {
        let m = ModelRef::parse_canonical("openai/gpt-5").expect("parses");
        assert_eq!(m.provider_id, "openai");
        assert_eq!(m.model, "gpt-5");
        // Nested ids keep the remainder as model.
        let m = ModelRef::parse_canonical("openrouter/anthropic/claude-sonnet").expect("parses");
        assert_eq!(m.provider_id, "openrouter");
        assert_eq!(m.model, "anthropic/claude-sonnet");
        assert!(ModelRef::parse_canonical("nonsense").is_none());
    }
}
