use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tauri::Emitter;
use uuid::Uuid;

use crate::agents::AgentRegistry;
use crate::dev_trace::{self, TraceEntry};
use crate::domain::agent::{AgentContext, AgentDefinition, AgentOutput};
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
                trace_agent_run("agent", &ctx, &def, input, started, Ok(&output));
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
                trace_agent_run("agent", &ctx, &def, input, started, Err(&e));
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
            .run_diagnostic_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture.clone(), scope)
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
                trace_agent_run("roadmap", &ctx, &def, input, started, Ok(&output));
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                // Same salvage contract as `run_notebook_block_agent`: a turn
                // cap firing while preparing the NEXT call means a prior tool
                // call may already have written into the capture — ANY
                // populated field is a deliverable the caller can apply.
                let salvaged = {
                    let c = capture.lock().unwrap_or_else(|err| err.into_inner());
                    c.assessment.is_some()
                        || c.diagnostic_battery.is_some()
                        || c.propose_syllabus_plan.is_some()
                        || c.propose_capstone_project.is_some()
                        || c.confirm_syllabus_plan.is_some()
                };
                if salvaged {
                    tracing::warn!(
                        run_id = %ctx.run_id,
                        error = %e,
                        "agent run failed after {} tool call already captured; salvaging captured output", ctx.agent_id
                    );
                    dev_trace::trace_note(
                        "salvage",
                        "agent_capture_salvaged",
                        &ctx.run_id,
                        "salvaged_after_error",
                        &[e.to_string()],
                    );
                    emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, ""));
                    return Ok(AgentOutput {
                        run_id: ctx.run_id.clone(),
                        agent_id: ctx.agent_id.clone(),
                        parent_run_id: None,
                        text: String::new(),
                        tool_calls: vec![],
                        duration_ms: started.elapsed().as_millis() as u64,
                    });
                }
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "diagnostic agent run failed"
                );
                trace_agent_run("roadmap", &ctx, &def, input, started, Err(&e));
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
            .run_notebook_block_execution(&def.model.provider_id, &def.model.model, &def.system_prompt, input, capture.clone())
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
                trace_agent_run("notebook_block", &ctx, &def, input, started, Ok(&output));
                emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, &output.text));
                Ok(output)
            }
            Err(e) => {
                // Rig's MaxTurnsError fires while PREPARING the next model
                // call — AFTER a prior turn's `publish_notebook_block` already
                // executed and wrote into the capture. Returning Err here
                // would discard a successfully published block, so a
                // populated capture is salvaged as a completed run.
                if capture.lock().unwrap_or_else(|err| err.into_inner()).is_some() {
                    tracing::warn!(
                        run_id = %ctx.run_id,
                        error = %e,
                        "agent run failed after {} tool call already published; salvaging captured output", ctx.agent_id
                    );
                    dev_trace::trace_note(
                        "salvage",
                        "agent_capture_salvaged",
                        &ctx.run_id,
                        "salvaged_after_error",
                        &[e.to_string()],
                    );
                    emit(app, AgentEvent::completed(&ctx.run_id, &ctx.agent_id, ""));
                    return Ok(AgentOutput {
                        run_id: ctx.run_id.clone(),
                        agent_id: ctx.agent_id.clone(),
                        parent_run_id: None,
                        text: String::new(),
                        tool_calls: vec![],
                        duration_ms: started.elapsed().as_millis() as u64,
                    });
                }
                tracing::warn!(
                    run_id = %ctx.run_id,
                    agent_id = %ctx.agent_id,
                    duration_ms = started.elapsed().as_millis() as u64,
                    error = %e,
                    "notebook block agent run failed"
                );
                trace_agent_run("notebook_block", &ctx, &def, input, started, Err(&e));
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
                trace_agent_run("gate_grading", &ctx, &def, input, started, Ok(&output));
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
                trace_agent_run("gate_grading", &ctx, &def, input, started, Err(&e));
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
                trace_agent_run("closure_grading", &ctx, &def, input, started, Ok(&output));
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
                trace_agent_run("closure_grading", &ctx, &def, input, started, Err(&e));
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
                trace_agent_run("pedagogical_critic", &ctx, &def, input, started, Ok(&output));
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
                trace_agent_run("pedagogical_critic", &ctx, &def, input, started, Err(&e));
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

/// Dev-mode capture of one agent run: the full system prompt, input, output
/// and (on failure) the classified error — the data the metadata-only
/// `tracing` logs deliberately omit. No-op (and no clones) while dev mode is off.
fn trace_agent_run(flow: &str, ctx: &AgentContext, def: &AgentDefinition, input: &str, started: Instant, outcome: Result<&AgentOutput, &AppError>) {
    if !dev_trace::is_enabled() {
        return;
    }
    let mut entry = TraceEntry::new("agent_run");
    entry.flow = Some(flow.to_string());
    entry.run_id = Some(ctx.run_id.clone());
    entry.agent_id = Some(ctx.agent_id.clone());
    entry.provider = Some(def.model.provider_id.clone());
    entry.model = Some(def.model.model.clone());
    entry.duration_ms = Some(started.elapsed().as_millis() as u64);
    entry.system_prompt = Some(def.system_prompt.clone());
    entry.input = Some(input.to_string());
    let entry = match outcome {
        Ok(out) => {
            entry.ok = Some(true);
            entry.output = Some(out.text.clone());
            entry.tool_calls = out.tool_calls.clone();
            entry
        }
        Err(e) => dev_trace::with_error(entry, e),
    };
    dev_trace::record(entry);
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
    use std::sync::Mutex;

    use super::*;
    use crate::agents::analyst_agent::ANALYST_AGENT_ID;
    use crate::agents::notebook_agent::NOTEBOOK_AGENT_ID;
    use crate::domain::agent::AgentDefinition;
    use crate::domain::model::ModelRef;
    use crate::domain::notebook::GeneratedSectionBlock;
    use crate::domain::roadmap::{DiagnosticAssessmentArgs, EntryLevel};
    use crate::providers::factory::PromptOutput;
    use crate::tools::{PublishedBlock, RoadmapCapture};

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

    fn max_turns_error() -> AppError {
        AppError::AgentExecutionFailed("MaxTurnsError: reached max turns limit: 6".to_string())
    }

    fn anchor_block() -> GeneratedSectionBlock {
        GeneratedSectionBlock::AnchoredMicroTheory {
            title: "La gran imagen".to_string(),
            intuitive_hook: "Piensa en una fábrica circular".to_string(),
            analogy_boundary: None,
            system_rule: "Explicación breve del tema.".to_string(),
            frequent_error: "Confundir la entrada con la salida".to_string(),
        }
    }

    /// Writes into the capture FIRST (simulating the tool call having
    /// already executed) and only then fails with rig's turn cap — the exact
    /// ordering that used to discard a successful deliverable.
    /// `write_capture: false` scripts the other variant: the model burned
    /// every turn without ever acting, so the capture stays empty.
    struct SalvageRunner {
        write_capture: bool,
    }

    #[async_trait]
    impl PromptRunner for SalvageRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
            unreachable!("specialized flows never call the plain envelope runner")
        }

        async fn run_notebook_block_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: NotebookBlockCapture,
        ) -> AppResult<PromptOutput> {
            if self.write_capture {
                *capture.lock().unwrap() = Some(PublishedBlock { block: anchor_block(), mechanical_template: false });
            }
            Err(max_turns_error())
        }

        async fn run_diagnostic_execution(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            capture: SharedRoadmapCapture,
            _scope: DiagnosticToolScope,
        ) -> AppResult<PromptOutput> {
            if self.write_capture {
                capture.lock().unwrap().assessment = Some(DiagnosticAssessmentArgs {
                    topic: "Ciclo de Krebs".to_string(),
                    target_goal: "Aprobar el examen".to_string(),
                    entry_level: EntryLevel::TheoreticalFoundations,
                    timeframe_weeks: 4,
                    weekly_commitment_hours: 3.0,
                });
            }
            Err(max_turns_error())
        }
    }

    #[tokio::test]
    async fn a_turn_cap_after_a_successful_publish_is_salvaged_instead_of_discarded() {
        let agents = agents_with(def(NOTEBOOK_AGENT_ID), def(ANALYST_AGENT_ID));
        let orch = Orchestrator::new(agents, Arc::new(SalvageRunner { write_capture: true }));
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));

        let out = orch.run_notebook_block_agent(None, NOTEBOOK_AGENT_ID, "go", capture.clone()).await.expect("salvaged");

        assert!(out.text.is_empty(), "salvage carries no model text; the capture IS the deliverable");
        assert!(!out.run_id.is_empty());
        assert!(capture.lock().unwrap().is_some(), "the published block must survive the turn-cap error");
    }

    #[tokio::test]
    async fn a_turn_cap_with_an_empty_capture_still_propagates_the_error() {
        let agents = agents_with(def(NOTEBOOK_AGENT_ID), def(ANALYST_AGENT_ID));
        let orch = Orchestrator::new(agents, Arc::new(SalvageRunner { write_capture: false }));
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));

        let err = orch
            .run_notebook_block_agent(None, NOTEBOOK_AGENT_ID, "go", capture.clone())
            .await
            .expect_err("nothing was captured, so the error must propagate");

        assert!(matches!(&err, AppError::AgentExecutionFailed(msg) if msg.contains("MaxTurnsError")));
        assert!(capture.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn a_diagnostic_turn_cap_after_a_captured_tool_call_is_salvaged() {
        let agents = agents_with(def(NOTEBOOK_AGENT_ID), def(ANALYST_AGENT_ID));
        let orch = Orchestrator::new(agents, Arc::new(SalvageRunner { write_capture: true }));
        let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));

        let out = orch
            .run_diagnostic_agent(None, ANALYST_AGENT_ID, "go", capture.clone(), DiagnosticToolScope::Assessment)
            .await
            .expect("salvaged");

        assert!(out.text.is_empty());
        assert!(capture.lock().unwrap().assessment.is_some(), "the submitted assessment must survive the turn-cap error");
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
