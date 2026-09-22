use serde::{Deserialize, Serialize};

/// Supported LLM providers (BYOK). The API key is deliberately NOT part of
/// this enum or of [`ProviderConfig`]; keys live only in the [`SecretVault`](crate::secrets::SecretVault).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Gemini,
    OpenRouter,
    DeepSeek,
}

impl ProviderKind {
    pub fn all() -> &'static [ProviderKind] {
        &[
            ProviderKind::OpenAI,
            ProviderKind::Anthropic,
            ProviderKind::Gemini,
            ProviderKind::OpenRouter,
            ProviderKind::DeepSeek,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OpenAI",
            ProviderKind::Anthropic => "Anthropic",
            ProviderKind::Gemini => "Gemini",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::DeepSeek => "DeepSeek",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "https://api.openai.com/v1",
            ProviderKind::Anthropic => "https://api.anthropic.com",
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com",
            ProviderKind::OpenRouter => "https://openrouter.ai/api/v1",
            // No /v1 suffix: rig-core's DeepSeek client builds request paths
            // (e.g. `/chat/completions`) directly off this base.
            ProviderKind::DeepSeek => "https://api.deepseek.com",
        }
    }

    /// Example model ids shown in the UI as placeholders. These are NOT
    /// functional requirements; any model id can be configured manually.
    pub fn suggested_models(&self) -> &'static [&'static str] {
        match self {
            ProviderKind::OpenAI => &["gpt-5", "gpt-4.1-mini"],
            ProviderKind::Anthropic => &["claude-sonnet-4-6", "claude-haiku-4-5"],
            ProviderKind::Gemini => &["gemini-2.5-pro", "gemini-2.5-flash"],
            ProviderKind::OpenRouter => &["anthropic/claude-sonnet-4", "openai/gpt-5"],
            ProviderKind::DeepSeek => &[
                "deepseek-v4-flash",
                "deepseek-v4-pro",
                "deepseek-chat",
                "deepseek-reasoner",
            ],
        }
    }
}

/// Non-sensitive provider configuration. Persisted as JSON. Never contains secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub provider: ProviderKind,
    pub name: String,
    pub default_model: Option<String>,
    pub base_url: Option<String>,
}

impl ProviderConfig {
    pub fn effective_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| self.provider.default_base_url().to_string())
    }
}

/// Provider config plus key-presence flag for the UI. The key itself is never exposed.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderWithStatus {
    pub id: String,
    pub provider: ProviderKind,
    pub name: String,
    pub default_model: Option<String>,
    pub base_url: Option<String>,
    pub configured: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_base_url_falls_back_to_default() {
        let cfg = ProviderConfig {
            id: "openai".into(),
            provider: ProviderKind::OpenAI,
            name: "OpenAI".into(),
            default_model: None,
            base_url: None,
        };
        assert_eq!(cfg.effective_base_url(), "https://api.openai.com/v1");
    }
}
