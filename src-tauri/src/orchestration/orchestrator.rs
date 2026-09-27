use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tauri::Emitter;
use uuid::Uuid;

use crate::agents::AgentRegistry;
use crate::domain::agent::{AgentContext, AgentOutput};
use crate::domain::message::AgentEvent;
use crate::error::{AppError, AppResult};
use crate::providers::{ProviderFactory, ProviderRegistry};
use crate::secrets::SecretVault;
use crate::tools::{BlockAuditCapture, ClosureFeedbackCapture, DiagnosticToolScope, GateGradingCapture, NotebookBlockCapture, SharedRoadmapCapture};

/// Sealed prompt execution behind a trait so the orchestrator can be tested
/// without API keys or network access.
#[async_trait]
pub trait PromptRunner: Send + Sync {
    async fn run(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        tools: &[String],
    ) -> AppResult<crate::providers::factory::PromptOutput>;

    /// Like [`run`](Self::run), but with a roadmap turn's fixed, stateful
    /// tool set (real native LLM tool-calling into `capture`) instead of
    /// the id-gated `tools` list. `scope` narrows which of the 4 roadmap
    /// tools are registered — the `diagnostic_analyst` agent only ever gets
    /// `submit_diagnostic_assessment`, while the main roadmap agent gets the
    /// full set. Used for every turn of
    /// [`RoadmapService`](crate::application::RoadmapService)'s diagnostic
    /// conversation, not just a post-confirmation step — there is no
    /// confirmation step. Defaults to an error so existing test doubles
    /// that only implement `run` keep compiling unchanged.
    async fn run_diagnostic_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        _input: &str,
        _capture: SharedRoadmapCapture,
        _scope: DiagnosticToolScope,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        Err(AppError::ToolNotFound("diagnostic tool-calling not supported by this runner".to_string()))
    }

    /// Like [`run_diagnostic_execution`](Self::run_diagnostic_execution),
    /// but for the notebook generator's single `publish_notebook_block`
    /// tool — ONE block per call, not the whole notebook (see
    /// `domain::notebook::GeneratedDynamicNotebook`, retired). Kept as its
    /// own method (rather than a generic capture type) for the same reason
    /// `run`/`run_diagnostic_execution` are already split: each specialized
    /// flow gets its own narrow, typed trait method instead of a shared
    /// "any tool set" abstraction.
    async fn run_notebook_block_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        _input: &str,
        _capture: NotebookBlockCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        Err(AppError::ToolNotFound("notebook block tool-calling not supported by this runner".to_string()))
    }

    /// Like [`run_notebook_block_execution`](Self::run_notebook_block_execution),
    /// but for `notebook_gate_grader`'s single `grade_gate_submission` tool —
    /// grades one free-text gate submission (and, on failure, produces its
    /// scaffold hint in the same call).
    async fn run_gate_grading_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        _input: &str,
        _capture: GateGradingCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        Err(AppError::ToolNotFound("gate grading tool-calling not supported by this runner".to_string()))
    }

    /// Like [`run_gate_grading_execution`](Self::run_gate_grading_execution),
    /// but for `closure_feedback_grader`'s single `grade_closure_submission`
    /// tool — grades the closing block's free-text student reflection and
    /// ALWAYS produces student-facing feedback (pass or fail).
    async fn run_closure_grading_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        _input: &str,
        _capture: ClosureFeedbackCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        Err(AppError::ToolNotFound("closure grading tool-calling not supported by this runner".to_string()))
    }

    /// Like [`run_gate_grading_execution`](Self::run_gate_grading_execution),
    /// but for `pedagogical_critic`'s single `submit_block_audit` tool —
    /// the semantic (LLM) review layer for one generated notebook block,
    /// running only after the deterministic guardrails already passed.
    async fn run_block_audit_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        _input: &str,
        _capture: BlockAuditCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        Err(AppError::ToolNotFound("block audit tool-calling not supported by this runner".to_string()))
    }
}

/// Production runner: resolves provider config + vault key, delegates to the
/// [`ProviderFactory`] (the only place Rig clients are built).
pub struct RigPromptRunner {
    providers: Arc<ProviderRegistry>,
    vault: Arc<dyn SecretVault>,
}

impl RigPromptRunner {
    pub fn new(providers: Arc<ProviderRegistry>, vault: Arc<dyn SecretVault>) -> Self {
        Self { providers, vault }
    }
}

#[async_trait]
impl PromptRunner for RigPromptRunner {
    async fn run(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        tools: &[String],
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        // The agent definition pins the model; it wins over the provider default.
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_prompt(&config, &api_key, system_prompt, input, tools).await
    }

    async fn run_diagnostic_execution(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        capture: SharedRoadmapCapture,
        scope: DiagnosticToolScope,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_diagnostic_prompt(&config, &api_key, system_prompt, input, capture, scope).await
    }

    async fn run_notebook_block_execution(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        capture: NotebookBlockCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_notebook_block_prompt(&config, &api_key, system_prompt, input, capture).await
    }

    async fn run_gate_grading_execution(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        capture: GateGradingCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_gate_grading_prompt(&config, &api_key, system_prompt, input, capture).await
    }

    async fn run_closure_grading_execution(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        capture: ClosureFeedbackCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_closure_grading_prompt(&config, &api_key, system_prompt, input, capture).await
    }

    async fn run_block_audit_execution(
        &self,
        provider_id: &str,
        model: &str,
        system_prompt: &str,
        input: &str,
        capture: BlockAuditCapture,
    ) -> AppResult<crate::providers::factory::PromptOutput> {
        let mut config = self.providers.get(provider_id)?;
        config.default_model = Some(model.to_string());
        let api_key = self
            .vault
            .get_provider_key(provider_id)?
            .ok_or_else(|| AppError::ProviderKeyMissing(provider_id.to_string()))?;
        ProviderFactory::run_block_audit_prompt(&config, &api_key, system_prompt, input, capture).await
    }
}

/// Minimal multi-agent orchestrator.
///
/// Supports `run_agent` and `delegate` today; the API (run ids, parent links,
/// event channel) is designed for sequential / parallel / supervisor patterns later.
pub struct Orchestrator {
    agents: Arc<AgentRegistry>,
    runner: Arc<dyn PromptRunner>,
}

impl Orchestrator {
    pub fn new(agents: Arc<AgentRegistry>, runner: Arc<dyn PromptRunner>) -> Self {
        Self { agents, runner }
    }

    pub fn with_rig_runner(
        agents: Arc<AgentRegistry>,
        providers: Arc<ProviderRegistry>,
        vault: Arc<dyn SecretVault>,
    ) -> Self {
        Self::new(agents, Arc::new(RigPromptRunner::new(providers, vault)))
    }

    /// Execute one agent. Emits `agent://started|completed|error` when `app` is provided.
    pub async fn run_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        parent_run_id: Option<String>,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: parent_run_id.clone(),
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, ctx.parent_run_id.as_deref()));

        let started = Instant::now();
        // Log metadata only: never prompts, keys, or full responses.
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            tools = ?def.tools,
            input_len = input.len(),
            "agent run started"
        );

        match self
            .runner
            .run(&def.model.provider_id, &def.model.model, &def.system_prompt, input, &def.tools)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: ctx.parent_run_id.clone(),
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    tool_calls = ?output.tool_calls,
                    output_len = output.text.len(),
                    "agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// Execute one agent with a roadmap turn's fixed tool set (real native
    /// tool-calling into `capture`), scoped by `scope` instead of
    /// `run_agent`'s id-gated `tools` list. Emits the same `agent://*` events.
    pub async fn run_diagnostic_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        capture: SharedRoadmapCapture,
        scope: DiagnosticToolScope,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: None,
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, None));

        let started = Instant::now();
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            "diagnostic agent run started"
        );

        match self
            .runner
            .run_diagnostic_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture, scope)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: None,
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    tool_calls = ?output.tool_calls,
                    "diagnostic agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "diagnostic agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// Execute one agent with the notebook block generator's single-tool
    /// contract — one `publish_notebook_block` call, producing ONE block.
    /// Same shape as [`run_diagnostic_agent`](Self::run_diagnostic_agent),
    /// one capture type over.
    pub async fn run_notebook_block_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        capture: NotebookBlockCapture,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: None,
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, None));

        let started = Instant::now();
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            "notebook block agent run started"
        );

        match self
            .runner
            .run_notebook_block_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: None,
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    "notebook block agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "notebook block agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// Execute the gate grading agent's single-tool contract — one
    /// `grade_gate_submission` call, grading one free-text submission (and,
    /// on failure, producing its scaffold hint in the same call). Same
    /// shape as [`run_notebook_block_agent`](Self::run_notebook_block_agent).
    pub async fn run_gate_grading_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        capture: GateGradingCapture,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: None,
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, None));

        let started = Instant::now();
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            "gate grading agent run started"
        );

        match self
            .runner
            .run_gate_grading_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: None,
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    "gate grading agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "gate grading agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// Execute the closure feedback grader's single-tool contract — one
    /// `grade_closure_submission` call, grading the closing block's
    /// free-text student reflection (always producing student-facing
    /// feedback, pass or fail). Same shape as
    /// [`run_gate_grading_agent`](Self::run_gate_grading_agent).
    pub async fn run_closure_grading_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        capture: ClosureFeedbackCapture,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: None,
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, None));

        let started = Instant::now();
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            "closure grading agent run started"
        );

        match self
            .runner
            .run_closure_grading_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: None,
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    "closure grading agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "closure grading agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// Execute the pedagogical critic's single-tool contract — one
    /// `submit_block_audit` call, reviewing ONE generated notebook block.
    /// Same shape as [`run_gate_grading_agent`](Self::run_gate_grading_agent).
    pub async fn run_pedagogical_critic_agent(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        input: &str,
        capture: BlockAuditCapture,
    ) -> AppResult<AgentOutput> {
        let def = self.agents.get(agent_id)?;
        let ctx = AgentContext {
            run_id: Uuid::new_v4().to_string(),
            agent_id: def.id.clone(),
            parent_run_id: None,
        };
        emit(app, AgentEvent::started(&ctx.run_id, &ctx.agent_id, None));

        let started = Instant::now();
        tracing::info!(
            run_id = %ctx.run_id,
            agent_id = %ctx.agent_id,
            provider = %def.model.provider_id,
            model = %def.model.model,
            "pedagogical critic agent run started"
        );

        match self
            .runner
            .run_block_audit_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture)
            .await
        {
            Ok(out) => {
                let output = AgentOutput {
                    run_id: ctx.run_id.clone(),
                    agent_id: ctx.agent_id.clone(),
                    parent_run_id: None,
                    text: out.text.clone(),
                    tool_calls: out.tool_calls.clone(),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                tracing::info!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = output.duration_ms,
                    "pedagogical critic agent run completed"
                );
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "pedagogical critic agent run failed"
                );
                emit(app, AgentEvent::failed(&ctx.run_id, &ctx.agent_id, &e.to_string()));
                Err(e)
            }
        }
    }

    /// MVP delegation: validate both agents, then execute `to_agent` with the
    /// parent link set. Later: handoffs, fan-out, supervisor policies.
    pub async fn delegate(
        &self,
        app: Option<&tauri::AppHandle>,
        from_agent_id: &str,
        to_agent_id: &str,
        task: &str,
    ) -> AppResult<AgentOutput> {
        // Validates the delegating agent exists (its run is the logical parent).
        self.agents.get(from_agent_id)?;
        let parent_run_id = Uuid::new_v4().to_string();
        tracing::info!(
            from_agent = %from_agent_id,
            to_agent = %to_agent_id,
            parent_run_id = %parent_run_id,
            "delegating"
        );
        self.run_agent(app, to_agent_id, task, Some(parent_run_id)).await
    }
}

fn emit(app: Option<&tauri::AppHandle>, event: AgentEvent) {
    if let Some(handle) = app {
        let channel = match event.event.as_str() {
            "started" => "agent://started",
            "completed" => "agent://completed",
            _ => "agent://error",
        };
        // Event delivery must never fail a run.
        let _ = handle.emit(channel, &event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::AgentDefinition;
    use crate::domain::model::ModelRef;
    use crate::providers::factory::PromptOutput;

    struct MockRunner {
        text: String,
    }

    #[async_trait]
    impl PromptRunner for MockRunner {
        async fn run(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            input: &str,
            _tools: &[String],
        ) -> AppResult<PromptOutput> {
            Ok(PromptOutput {
                text: format!("{} | {input}", self.text),
                tool_calls: vec![],
            })
        }
    }

    #[tokio::test]
    async fn run_agent_returns_output() {
        let agents = agents_with(def("researcher"), def("writer"));
        let orch = Orchestrator::new(agents, Arc::new(MockRunner { text: "done".to_string() }));
        let out = orch.run_agent(None, "researcher", "task", None).await.expect("run");
        assert_eq!(out.agent_id, "researcher");
        assert!(out.text.contains("done"));
        assert!(!out.run_id.is_empty());
        assert!(out.parent_run_id.is_none());
    }

    #[tokio::test]
    async fn delegate_links_parent_run() {
        let agents = agents_with(def("researcher"), def("writer"));
        let orch = Orchestrator::new(agents, Arc::new(MockRunner { text: "ok".to_string() }));
        let out = orch.delegate(None, "researcher", "writer", "write it").await.expect("delegate");
        assert_eq!(out.agent_id, "writer");
        assert!(out.parent_run_id.is_some(), "delegation must set parent_run_id");
        assert_ne!(out.parent_run_id.as_deref(), Some(out.run_id.as_str()));
    }

    #[tokio::test]
    async fn unknown_agents_fail_typed() {
        let agents = Arc::new(AgentRegistry::new());
        let orch = Orchestrator::new(agents, Arc::new(MockRunner { text: "x".to_string() }));
        assert!(matches!(
            orch.run_agent(None, "ghost", "hi", None).await,
            Err(AppError::AgentNotFound(_))
        ));
        assert!(matches!(
            orch.delegate(None, "ghost", "writer", "hi").await,
            Err(AppError::AgentNotFound(_))
        ));
    }
    fn agents_with(a: AgentDefinition, b: AgentDefinition) -> Arc<AgentRegistry> {
        let r = Arc::new(AgentRegistry::new());
        r.register(a).expect("a");
        r.register(b).expect("b");
        r
    }

    fn def(id: &str) -> AgentDefinition {
        AgentDefinition {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            system_prompt: "sys".to_string(),
            model: ModelRef::new("openai", "gpt-5"),
            tools: vec![],
        }
    }
}
