use rig_agent::AgentBuilder;
use rig_agent::completion::Prompt;
use rig_core::client::CompletionClient;

use crate::domain::model::ModelInfo;
use crate::domain::provider::{ProviderConfig, ProviderKind};
use crate::error::{AppError, AppResult};
use crate::tools::{EchoTool, is_known_tool};

/// Output of a single prompt execution.
#[derive(Debug, Clone)]
pub struct PromptOutput {
    pub text: String,
    /// Tools available to this run. Per-call attribution is a hook-level
    /// extension point (see `orchestrator`); the MVP reports availability.
    pub tool_calls: Vec<String>,
}

/// Centralized factory: the ONLY place that turns
/// `provider + api_key + model` into a usable Rig agent.
///
/// To add a provider: extend [`ProviderKind`], add one match arm in each
/// method below, done. No UI or registry changes required.
pub struct ProviderFactory;

impl ProviderFactory {
    /// Resolve which model id to use: explicit override first, else the
    /// provider's default. Never falls back to a hardcoded model.
    pub fn resolve_model(config: &ProviderConfig, override_model: Option<&str>) -> AppResult<String> {
        if let Some(m) = override_model.filter(|m| !m.trim().is_empty()) {
            return Ok(m.to_string());
        }
        config
            .default_model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .ok_or_else(|| AppError::ModelNotConfigured(config.id.clone()))
    }

    fn require_key(api_key: &str, provider_id: &str) -> AppResult<()> {
        if api_key.trim().is_empty() {
            return Err(AppError::ProviderKeyMissing(provider_id.to_string()));
        }
        Ok(())
    }

    /// Build a Rig agent and run one prompt with optional tools.
    pub async fn run_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        tools: &[String],
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        for tool_id in tools {
            if !is_known_tool(tool_id) {
                return Err(AppError::ToolNotFound(tool_id.clone()));
            }
        }
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| AppError::ProviderConnectionFailed(format!("build client: {e}")))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| AppError::ProviderConnectionFailed(format!("build client: {e}")))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| AppError::ProviderConnectionFailed(format!("build client: {e}")))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| AppError::ProviderConnectionFailed(format!("build client: {e}")))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| AppError::ProviderConnectionFailed(format!("build client: {e}")))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
        }
    }

    async fn prompt_with<M>(model: M, system_prompt: &str, input: &str, tools: &[String]) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        let use_echo = tools.iter().any(|t| t == "echo");
        let builder = AgentBuilder::new(model).preamble(system_prompt);
        let agent = if use_echo {
            builder.tool(EchoTool).build()
        } else {
            builder.build()
        };
        let text = agent
            .prompt(input)
            .await
            .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput {
            text,
            tool_calls: if use_echo { vec!["echo".to_string()] } else { vec![] },
        })
    }

    /// Lightweight connectivity check (no tokens spent, no prompt sent).
    pub async fn verify_connection(config: &ProviderConfig, api_key: &str) -> AppResult<()> {
        Self::require_key(api_key, &config.id)?;
        let http = reqwest::Client::new();
        let base_url = config.effective_base_url();

        let response = match config.provider {
            ProviderKind::OpenAI | ProviderKind::OpenRouter | ProviderKind::DeepSeek => {
                let mut req = http
                    .get(format!("{}/models", base_url.trim_end_matches('/')))
                    .bearer_auth(api_key);
                if config.provider == ProviderKind::OpenRouter {
                    req = req
                        .header("HTTP-Referer", "http://localhost")
                        .header("X-Title", "LearnKit");
                }
                req.send().await
            }
            ProviderKind::Anthropic => {
                http.get(format!("{}/v1/models", base_url.trim_end_matches('/')))
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .send()
                    .await
            }
            ProviderKind::Gemini => {
                http.get(format!(
                    "{}/v1beta/models",
                    base_url.trim_end_matches('/')
                ))
                .query(&[("key", api_key)])
                .send()
                .await
            }
        }
        .map_err(|_| AppError::ProviderConnectionFailed("unreachable".to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(AppError::ProviderConnectionFailed(format!(
                "status {}",
                response.status()
            )))
        }
    }

    /// Best-effort model discovery. The UI always allows manual model ids,
    /// since discovery is not reliable for every provider.
    pub async fn list_models(config: &ProviderConfig, api_key: &str) -> AppResult<Vec<ModelInfo>> {
        Self::require_key(api_key, &config.id)?;
        let http = reqwest::Client::new();
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI | ProviderKind::OpenRouter | ProviderKind::DeepSeek => {
                let mut req = http
                    .get(format!("{}/models", base_url.trim_end_matches('/')))
                    .bearer_auth(api_key);
                if config.provider == ProviderKind::OpenRouter {
                    req = req.header("HTTP-Referer", "http://localhost").header("X-Title", "LearnKit");
                }
                let res = req.send().await.map_err(|_| AppError::ProviderConnectionFailed("unreachable".to_string()))?;
                if !res.status().is_success() {
                    return Err(AppError::ProviderConnectionFailed(format!("status {}", res.status())));
                }
                let body: serde_json::Value = res.json().await.map_err(|_| AppError::ProviderConnectionFailed("bad response".to_string()))?;
                Ok(body
                    .get("data")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                            .map(|id| ModelInfo { id: id.to_string(), name: None })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            ProviderKind::Anthropic => {
                let res = http
                    .get(format!("{}/v1/models", base_url.trim_end_matches('/')))
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .send()
                    .await
                    .map_err(|_| AppError::ProviderConnectionFailed("unreachable".to_string()))?;
                if !res.status().is_success() {
                    return Err(AppError::ProviderConnectionFailed(format!("status {}", res.status())));
                }
                let body: serde_json::Value = res.json().await.map_err(|_| AppError::ProviderConnectionFailed("bad response".to_string()))?;
                Ok(body
                    .get("data")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                            .map(|id| ModelInfo {
                                id: id.to_string(),
                                name: body_name_lookup(&body, id),
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            ProviderKind::Gemini => {
                let res = http
                    .get(format!("{}/v1beta/models", base_url.trim_end_matches('/')))
                    .query(&[("key", api_key)])
                    .send()
                    .await
                    .map_err(|_| AppError::ProviderConnectionFailed("unreachable".to_string()))?;
                if !res.status().is_success() {
                    return Err(AppError::ProviderConnectionFailed(format!("status {}", res.status())));
                }
                let body: serde_json::Value = res.json().await.map_err(|_| AppError::ProviderConnectionFailed("bad response".to_string()))?;
                Ok(body
                    .get("models")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                            .map(|name| ModelInfo {
                                id: name.trim_start_matches("models/").to_string(),
                                name: None,
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
        }
    }
}

fn body_name_lookup(_body: &serde_json::Value, _id: &str) -> Option<String> {
    None
}

/// Strip any accidental secret-looking material from provider errors before
/// they reach the UI/logs. Provider SDK errors should not contain keys, but
/// URLs and bearer tokens are redacted defensively.
fn trim_error(msg: &str) -> String {
    const LIMIT: usize = 500;
    let mut out = msg.chars().take(LIMIT).collect::<String>();
    for token in ["sk-", "Bearer ", "key="] {
        if let Some(idx) = out.find(token) {
            out = format!("{}[redacted]", &out[..idx]);
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openai_cfg() -> ProviderConfig {
        ProviderConfig {
            id: "openai".to_string(),
            provider: ProviderKind::OpenAI,
            name: "OpenAI".to_string(),
            default_model: None,
            base_url: None,
        }
    }

    #[test]
    fn resolve_model_prefers_override_then_default() {
        let mut cfg = openai_cfg();
        assert!(matches!(
            ProviderFactory::resolve_model(&cfg, None),
            Err(AppError::ModelNotConfigured(_))
        ));
        cfg.default_model = Some("gpt-5".to_string());
        assert_eq!(ProviderFactory::resolve_model(&cfg, None).expect("ok"), "gpt-5");
        assert_eq!(
            ProviderFactory::resolve_model(&cfg, Some("other")).expect("ok"),
            "other"
        );
    }

    #[tokio::test]
    async fn rejects_empty_key_without_network() {
        let cfg = openai_cfg();
        let err = ProviderFactory::verify_connection(&cfg, "   ").await.expect_err("must fail");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
        let err = ProviderFactory::list_models(&cfg, "").await.expect_err("must fail");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
    }

    #[tokio::test]
    async fn rejects_unknown_tool_without_network() {
        let mut cfg = openai_cfg();
        cfg.default_model = Some("gpt-5".to_string());
        let err = ProviderFactory::run_prompt(&cfg, "sk-test", "sys", "hi", &["nope".to_string()])
            .await
            .expect_err("must fail");
        assert!(matches!(err, AppError::ToolNotFound(_)));
    }

    #[test]
    fn error_trimming_redacts_bearer() {
        let msg = trim_error("401 Unauthorized Bearer sk-abc123 tail");
        assert!(!msg.contains("sk-abc123"));
        assert!(msg.contains("[redacted]"));
    }
}
