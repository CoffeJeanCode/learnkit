use std::path::PathBuf;
use std::sync::Arc;

use crate::agents::roadmap_agent::ROADMAP_AGENT_ID;
use crate::agents::{AgentRegistry, basic_agent};
use crate::application::{AgentService, ProviderService, RoadmapService, WorkflowService};
use crate::domain::model::ModelRef;
use crate::error::AppResult;
use crate::orchestration::Orchestrator;
use crate::persistence::FileStore;
use crate::providers::ProviderRegistry;
use crate::secrets::{SecretVault, open_vault_or_memory};

/// Shared, thread-safe application state (no global mutables).
pub struct AppState {
    pub providers: Arc<ProviderRegistry>,
    pub agents: Arc<AgentRegistry>,
    pub vault: Arc<dyn SecretVault>,
    pub vault_backend: String,
    pub orchestrator: Arc<Orchestrator>,
    pub provider_service: ProviderService,
    pub agent_service: AgentService,
    pub workflow_service: WorkflowService,
    pub roadmap_service: RoadmapService,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        let store = FileStore::new(data_dir.join("learnkit"));
        let (vault_box, backend) = open_vault_or_memory(&data_dir.join("learnkit"));
        let backend_name = if backend { "stronghold" } else { "memory" };
        let vault: Arc<dyn SecretVault> = vault_box.into();
        // Plug-and-play: pick up whatever provider key the developer already
        // has exported (same env var names the providers' own SDKs use), so
        // the app can run without a trip through the Providers UI first.
        seed_keys_from_env(vault.as_ref());

        let providers = Arc::new(ProviderRegistry::with_defaults());
        if let Ok(saved) = store.load_providers() {
            if !saved.is_empty() {
                providers.replace_all(saved);
            }
        }

        let agents = Arc::new(AgentRegistry::new());
        match store.load_agents() {
            Ok(saved) if !saved.is_empty() => agents.replace_all(saved),
            _ => {
                for def in basic_agent::demo_definitions() {
                    // Demo defs are valid by construction; ignore errors.
                    let _ = agents.register(def);
                }
                let _ = store.save_agents(&agents.list());
            }
        }

        let orchestrator = Arc::new(Orchestrator::with_rig_runner(
            Arc::clone(&agents),
            Arc::clone(&providers),
            Arc::clone(&vault),
        ));

        let provider_service =
            ProviderService::new(Arc::clone(&providers), Arc::clone(&vault), store.clone());
        let agent_service =
            AgentService::new(Arc::clone(&agents), Arc::clone(&orchestrator), store.clone());
        let workflow_service = WorkflowService::new(Arc::clone(&orchestrator));
        let roadmap_service = RoadmapService::new(Arc::clone(&orchestrator), store.clone());

        tracing::info!(vault_backend = %backend_name, "app state initialized");

        let state = Self {
            providers,
            agents,
            vault,
            vault_backend: backend_name.to_string(),
            orchestrator,
            provider_service,
            agent_service,
            workflow_service,
            roadmap_service,
            data_dir,
        };
        // Covers both first run (fresh demo defs) and a stale `agents.json`
        // from before any key existed.
        state.heal_roadmap_agent_model();
        Ok(state)
    }

    /// Re-points the roadmap agent at an available provider if the one it's
    /// currently assigned to still has no key. Called at startup, and again
    /// by the `save_provider_key` command right after a key is saved — so
    /// the onboarding sequence (pick provider -> save key -> start) always
    /// ends with a working agent instead of a stale pin to an unconfigured
    /// provider.
    pub fn heal_roadmap_agent_model(&self) {
        let Ok(roadmap_def) = self.agents.get(ROADMAP_AGENT_ID) else { return };
        if self.vault.has_provider_key(&roadmap_def.model.provider_id).unwrap_or(false) {
            return;
        }
        let Some(model) = pick_available_model(self.vault.as_ref()) else { return };
        let mut updated = roadmap_def;
        updated.model = model;
        if self.agents.register(updated).is_ok() {
            let _ = self.agent_service.persist();
        }
    }
}

/// `(provider_id, env var names to check in order)`. Uses the same env var
/// names each provider's own SDK/CLI conventionally reads, so a developer
/// who already has e.g. `ANTHROPIC_API_KEY` exported needs to do nothing.
const ENV_KEY_CANDIDATES: &[(&str, &[&str])] = &[
    ("anthropic", &["ANTHROPIC_API_KEY"]),
    ("deepseek", &["DEEPSEEK_API_KEY"]),
    ("openai", &["OPENAI_API_KEY"]),
    ("gemini", &["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
    ("openrouter", &["OPENROUTER_API_KEY"]),
];

/// Seeds vault keys from environment variables, skipping any provider that
/// already has one. Safe to call on every startup: it never overwrites a key
/// the user set through the Providers UI.
fn seed_keys_from_env(vault: &dyn SecretVault) {
    for (provider_id, env_names) in ENV_KEY_CANDIDATES {
        if vault.has_provider_key(provider_id).unwrap_or(false) {
            continue;
        }
        for env_name in *env_names {
            let Ok(key) = std::env::var(env_name) else { continue };
            if key.trim().is_empty() {
                continue;
            }
            if vault.save_provider_key(provider_id, &key).is_ok() {
                tracing::info!(provider_id, env_name, "auto-configured provider key from environment");
            }
            break;
        }
    }
}

/// Same priority order as [`ENV_KEY_CANDIDATES`]: the first provider that
/// actually has a key wins, so the roadmap agent's first-run default model
/// matches whatever the developer configured rather than a fixed guess.
fn pick_available_model(vault: &dyn SecretVault) -> Option<ModelRef> {
    const MODEL_FOR: &[(&str, &str)] = &[
        ("anthropic", "claude-sonnet-4-6"),
        ("deepseek", "deepseek-v4-flash"),
        ("openai", "gpt-5"),
        ("gemini", "gemini-2.5-flash"),
        ("openrouter", "anthropic/claude-sonnet-4"),
    ];
    MODEL_FOR
        .iter()
        .find(|(provider_id, _)| vault.has_provider_key(provider_id).unwrap_or(false))
        .map(|(provider_id, model)| ModelRef::new(*provider_id, *model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryVault;

    #[test]
    fn seeds_key_from_env_only_when_vault_has_none() {
        // SAFETY: `cargo test` runs this crate's tests single-threaded by
        // default is NOT guaranteed, but env var names here are unique to
        // this test and not read elsewhere in the suite.
        unsafe { std::env::set_var("DEEPSEEK_API_KEY", "sk-env-test") };
        let vault = MemoryVault::new();

        seed_keys_from_env(&vault);
        assert_eq!(vault.get_provider_key("deepseek").unwrap().as_deref(), Some("sk-env-test"));

        // A key already in the vault must never be overwritten by the env var.
        vault.save_provider_key("deepseek", "sk-manual").unwrap();
        seed_keys_from_env(&vault);
        assert_eq!(vault.get_provider_key("deepseek").unwrap().as_deref(), Some("sk-manual"));

        unsafe { std::env::remove_var("DEEPSEEK_API_KEY") };
    }

    #[test]
    fn picks_first_available_provider_in_priority_order() {
        let vault = MemoryVault::new();
        assert!(pick_available_model(&vault).is_none());

        vault.save_provider_key("gemini", "k").unwrap();
        assert_eq!(pick_available_model(&vault).unwrap().provider_id, "gemini");

        // Anthropic outranks Gemini once both are configured.
        vault.save_provider_key("anthropic", "k").unwrap();
        assert_eq!(pick_available_model(&vault).unwrap().provider_id, "anthropic");
    }
}
