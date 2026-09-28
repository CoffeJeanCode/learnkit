use std::path::PathBuf;
use std::sync::Arc;

use crate::agents::roadmap_agent::ROADMAP_AGENT_ID;
use crate::agents::{AgentRegistry, basic_agent};
use crate::application::{AgentService, LexicalAssistantService, NotebookService, ProviderService, RoadmapService, WorkflowService};
use crate::domain::model::ModelRef;
use crate::error::AppResult;
use crate::notebook_store::NotebookStore;
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
    pub notebook_service: NotebookService,
    pub lexical_assistant_service: LexicalAssistantService,
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
        // The roadmap agent is a fixed, non-user-editable built-in (there's
        // no UI to author its system prompt) — always overwrite whatever
        // was persisted with the current code's definition, so a prompt
        // rewrite in a new build can never be shadowed by a stale
        // `agents.json` entry from before the rewrite. `.model` gets
        // re-pinned right after by `heal_roadmap_agent_model` if the
        // default provider here has no key.
        if agents.register(crate::agents::roadmap_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::notebook_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::notebook_gate_grader_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::closure_feedback_grader_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::analyst_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::pedagogical_critic_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }
        if agents.register(crate::agents::lexical_assistant_agent::definition()).is_ok() {
            let _ = store.save_agents(&agents.list());
        }

        let notebook_store = NotebookStore::open(data_dir.join("learnkit").join("notebook.sqlite3"))?;

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
        let roadmap_service =
            RoadmapService::new(Arc::clone(&orchestrator), store.clone(), notebook_store.clone());
        // Cleanup for the "borré la sesión y las clases seguían ahí" bug:
        // courses whose owning session is already gone (deleted before
        // delete_session cascaded) are removed on every launch, so the
        // "Clases" section can never show a course no session backs.
        match roadmap_service.sweep_orphan_courses() {
            Ok(0) => {}
            Ok(n) => tracing::info!(removed_courses = n, "swept courses orphaned by deleted sessions"),
            Err(e) => tracing::warn!(error = %e, "orphan course sweep failed (non-fatal)"),
        }
        let notebook_service = NotebookService::new(notebook_store, Arc::clone(&orchestrator));
        let lexical_assistant_service = LexicalAssistantService::new(Arc::clone(&orchestrator));

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
            notebook_service,
            lexical_assistant_service,
            data_dir,
        };
        // Covers both first run (fresh demo defs) and a stale `agents.json`
        // from before any key existed.
        state.heal_roadmap_agent_model();
        Ok(state)
    }

    /// Re-points the roadmap agent (and its execution-only siblings: the
    /// notebook block generator, the gate grader, the closure feedback
    /// grader, the lexical assistant) at an available provider if the one
    /// currently assigned has no key. Called at startup, and again by the
    /// `save_provider_key` command right after a key is saved — so the
    /// onboarding sequence (pick provider -> save key -> start) always ends
    /// with working agents instead of a stale pin to an unconfigured
    /// provider.
    pub fn heal_roadmap_agent_model(&self) {
        self.heal_agent_model(ROADMAP_AGENT_ID);
        self.heal_agent_model(crate::agents::notebook_agent::NOTEBOOK_AGENT_ID);
        self.heal_agent_model(crate::agents::notebook_gate_grader_agent::NOTEBOOK_GATE_GRADER_AGENT_ID);
        self.heal_agent_model(crate::agents::closure_feedback_grader_agent::CLOSURE_FEEDBACK_GRADER_AGENT_ID);
        self.heal_agent_model(crate::agents::analyst_agent::ANALYST_AGENT_ID);
        self.heal_agent_model(crate::agents::pedagogical_critic_agent::PEDAGOGICAL_CRITIC_AGENT_ID);
        self.heal_agent_model(crate::agents::lexical_assistant_agent::LEXICAL_ASSISTANT_AGENT_ID);
    }

    fn heal_agent_model(&self, agent_id: &str) {
        let Ok(def) = self.agents.get(agent_id) else { return };
        if self.vault.has_provider_key(&def.model.provider_id).unwrap_or(false) {
            return;
        }
        let Some(model) = pick_available_model(self.vault.as_ref()) else { return };
        let mut updated = def;
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
/// the user set through the Providers UI. Collects first and persists once —
/// each Stronghold persist re-encrypts the whole snapshot.
fn seed_keys_from_env(vault: &dyn SecretVault) {
    let mut batch: Vec<(String, String)> = Vec::new();
    for (provider_id, env_names) in ENV_KEY_CANDIDATES {
        if vault.has_provider_key(provider_id).unwrap_or(false) {
            continue;
        }
        for env_name in *env_names {
            let Ok(key) = std::env::var(env_name) else { continue };
            if key.trim().is_empty() {
                continue;
            }
            batch.push((provider_id.to_string(), key));
            break;
        }
    }
    if !batch.is_empty() {
        // Provider ids only — never key material.
        let ids: Vec<&str> = batch.iter().map(|(id, _)| id.as_str()).collect();
        if vault.save_provider_keys(&batch).is_ok() {
            tracing::info!(provider_ids = ?ids, "auto-configured provider keys from environment");
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
    fn stale_persisted_roadmap_prompt_is_overwritten_on_startup() {
        // Regression test: a real user's `agents.json` was found on disk
        // still holding the roadmap agent's OLD (pre-redesign) system
        // prompt, because agent loading only regenerates demo defs when the
        // file is empty — a stale roadmap agent entry was silently trusted
        // forever. `AppState::new` must now always win with the current
        // code's prompt for this one built-in agent.
        let data_dir = std::env::temp_dir().join(format!("learnkit-appstate-{}", uuid::Uuid::new_v4()));
        let store = crate::persistence::FileStore::new(data_dir.join("learnkit"));
        let mut stale = crate::agents::roadmap_agent::definition();
        stale.system_prompt = "STALE PROMPT FROM BEFORE THE REDESIGN".to_string();
        store.save_agents(&[stale]).expect("seed stale agents.json");

        let state = AppState::new(data_dir.clone()).expect("app state initializes");
        let current = state.agents.get(ROADMAP_AGENT_ID).expect("roadmap agent registered");
        assert_eq!(current.system_prompt, crate::agents::roadmap_agent::definition().system_prompt);

        // And it must have persisted the fix back to disk too, not just in
        // the in-memory registry.
        let reloaded = store.load_agents().expect("reload agents.json");
        let persisted = reloaded.iter().find(|a| a.id == ROADMAP_AGENT_ID).expect("roadmap agent persisted");
        assert_eq!(persisted.system_prompt, crate::agents::roadmap_agent::definition().system_prompt);

        let _ = std::fs::remove_dir_all(&data_dir);
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

    #[test]
    fn the_closure_feedback_grader_heals_onto_a_provider_that_has_a_key() {
        // Regression: the grader shipped pinned to `anthropic` while every
        // other execution-only agent was on the heal list — a user whose
        // only configured key was a different provider hit
        // `ProviderKeyMissing("anthropic")` on every closure feedback call.
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
        let data_dir = std::env::temp_dir().join(format!("learnkit-appstate-heal-{}", uuid::Uuid::new_v4()));
        let state = AppState::new(data_dir.clone()).expect("app state initializes");

        state.vault.save_provider_key("gemini", "k").expect("configure a key");
        state.heal_roadmap_agent_model();

        let grader = state
            .agents
            .get(crate::agents::closure_feedback_grader_agent::CLOSURE_FEEDBACK_GRADER_AGENT_ID)
            .expect("closure feedback grader registered");
        assert_ne!(grader.model.provider_id, "anthropic", "must not stay pinned to the unconfigured default");
        assert!(
            state.vault.has_provider_key(&grader.model.provider_id).unwrap_or(false),
            "re-pinned onto a provider that actually has a key ({})",
            grader.model.provider_id
        );

        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
