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
    Xiaomi,
}

impl ProviderKind {
    pub fn all() -> &'static [ProviderKind] {
        &[
            ProviderKind::OpenAI,
            ProviderKind::Anthropic,
            ProviderKind::Gemini,
            ProviderKind::OpenRouter,
            ProviderKind::DeepSeek,
            ProviderKind::Xiaomi,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OpenAI",
            ProviderKind::Anthropic => "Anthropic",
            ProviderKind::Gemini => "Gemini",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::DeepSeek => "DeepSeek",
            ProviderKind::Xiaomi => "Xiaomi MiMo",
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
            ProviderKind::Xiaomi => "https://api.xiaomimimo.com/v1",
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
            ProviderKind::Xiaomi => &["mimo-v2.6-pro", "mimo-v2.6-flash", "mimo-v2.6-pro-ultraspeed"],
        }
    }

    /// A faster/cheaper model of the same provider for low-stakes agents (the
    /// pedagogical critic, the lexical assistant) whose output is short and
    /// whose verdict never grades the student. `None` = no known fast sibling
    /// (use the main model).
    pub fn fast_model(&self) -> Option<&'static str> {
        match self {
            ProviderKind::OpenAI => Some("gpt-4.1-mini"),
            ProviderKind::Anthropic => Some("claude-haiku-4-5"),
            ProviderKind::Gemini => Some("gemini-2.5-flash"),
            ProviderKind::DeepSeek => Some("deepseek-v4-flash"),
            ProviderKind::Xiaomi => Some("mimo-v2.6-flash"),
            ProviderKind::OpenRouter => None,
        }
    }

    /// The model a LIGHT agent should run on given the user's main model.
    /// The fast sibling is used only when the main model is one of this
    /// provider's known suggestions: a custom model id means we can't know
    /// the sibling is available on that account, so the main model wins.
    pub fn light_agent_model(&self, main_model: &str) -> String {
        match self.fast_model() {
            Some(fast) if self.suggested_models().contains(&main_model) => fast.to_string(),
            _ => main_model.to_string(),
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
    /// The provider the built-in agents are currently pinned to.
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_agents_use_the_fast_sibling_only_for_a_known_main_model() {
        assert_eq!(ProviderKind::Anthropic.light_agent_model("claude-sonnet-4-6"), "claude-haiku-4-5");
        assert_eq!(ProviderKind::DeepSeek.light_agent_model("deepseek-v4-pro"), "deepseek-v4-flash");
        // A custom model id: the sibling may not exist on this account.
        assert_eq!(ProviderKind::Anthropic.light_agent_model("my-private-finetune"), "my-private-finetune");
        // No known fast sibling.
        assert_eq!(ProviderKind::OpenRouter.light_agent_model("openai/gpt-5"), "openai/gpt-5");
    }

    #[test]
    fn every_fast_model_is_a_suggested_model_and_a_fixed_point() {
        for kind in ProviderKind::all() {
            if let Some(fast) = kind.fast_model() {
                assert!(kind.suggested_models().contains(&fast), "{kind:?}: fast model {fast} must be a known suggestion");
                // Choosing the fast model as MAIN must leave light agents on it.
                assert_eq!(kind.light_agent_model(fast), fast);
            }
        }
    }

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
