use rig_agent::AgentBuilder;
use rig_agent::completion::Prompt;
use rig_core::client::CompletionClient;

use crate::domain::model::ModelInfo;
use crate::domain::provider::{ProviderConfig, ProviderKind};
use crate::error::{AppError, AppResult};
use crate::tools::{
    BlockAuditCapture, ClosureFeedbackCapture, ConfirmSyllabusPlanTool, DiagnosticToolScope, EchoTool, GateGradingCapture,
    GradeClosureSubmissionTool, GradeGateSubmissionTool, NotebookBlockCapture, PresentDiagnosticBatteryTool,
    ProposeCapstoneProjectTool, ProposeSyllabusPlanTool, PublishNotebookBlockTool, SharedRoadmapCapture, SubmitBlockAuditTool,
    SubmitDiagnosticAssessmentTool, is_known_tool,
};

/// Límite para una llamada al LLM: sin esto, una red colgada o un proveedor
/// lento deja el comando Tauri pendiente para siempre y la ventana parece
/// "no responder" (el composer queda deshabilitado en `sending` sin salida).
const PROMPT_TIMEOUT_SECS: u64 = 120;
/// Límite para los chequeos HTTP ligeros (verificar conexión, listar modelos).
const HTTP_TIMEOUT_SECS: u64 = 30;
/// Presupuesto de salida por defecto: cubre texto + args de herramientas de
/// los turnos chicos (preguntas, echo). Los turnos que emiten artefactos
/// gigantes (assessment + syllabus + notebook) usan `output_budget`.
const PLAIN_MAX_TOKENS: u64 = 8192;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

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

    /// Presupuesto de salida según proveedor. Los turnos de ejecución emiten
    /// artefactos enormes (un syllabus de 4 semanas + notebook completo
    /// caben justos en 8192 — observado `finish_reason=Length`), así que se
    /// pide 16384 donde el proveedor lo admite. DeepSeek capa a 8192 en su
    /// API: pedir más falla del lado del proveedor.
    pub fn output_budget(provider: &ProviderKind) -> u64 {
        match provider {
            ProviderKind::DeepSeek => 8192,
            _ => 16384,
        }
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
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with(model, system_prompt, input, tools).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
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
        let builder = AgentBuilder::new(model).preamble(system_prompt).max_tokens(PLAIN_MAX_TOKENS);
        let agent = if use_echo {
            builder.tool(EchoTool).build()
        } else {
            builder.build()
        };
        let text = tokio::time::timeout(std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS), agent.prompt(input))
            .await
            .map_err(|_| {
                AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s"))
            })?
            .map_err(|e| agent_error(&e))?;
        Ok(PromptOutput {
            text,
            tool_calls: if use_echo { vec!["echo".to_string()] } else { vec![] },
        })
    }

    /// Build a Rig agent with the roadmap agent's 2 tools registered (real
    /// native LLM tool-calling, not the `tools: &[String]` id-gated path
    /// `run_prompt` uses) and run one turn. The model may call neither (a
    /// plain clarifying question), one, or both in sequence — see
    /// `RoadmapService::advance`.
    pub async fn run_diagnostic_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: SharedRoadmapCapture,
        scope: DiagnosticToolScope,
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_diagnostic_tools(model, system_prompt, input, capture, Self::output_budget(&config.provider), scope).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_diagnostic_tools(model, system_prompt, input, capture, Self::output_budget(&config.provider), scope).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_diagnostic_tools(model, system_prompt, input, capture, Self::output_budget(&config.provider), scope).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_diagnostic_tools(model, system_prompt, input, capture, Self::output_budget(&config.provider), scope).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_diagnostic_tools(model, system_prompt, input, capture, Self::output_budget(&config.provider), scope).await
            }
        }
    }

    async fn prompt_with_diagnostic_tools<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: SharedRoadmapCapture,
        max_tokens: u64,
        scope: DiagnosticToolScope,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // Rig's builder is typestate-based: the FIRST `.tool()` call moves it
        // from `NoToolConfig` to `WithBuilderTools`, so a runtime-conditional
        // `if scope... { builder = builder.tool(...) }` won't typecheck. Each
        // scope registers its subset in its own branch instead — every scope
        // has at least one tool, so every branch ends in `.tool(...)`.
        let base = AgentBuilder::new(model)
            .preamble(system_prompt)
            // Presupuesto por proveedor (ver `output_budget`): un turno puede
            // emitir texto + 2 tool calls encadenados, el segundo cargando un
            // syllabus completo + notebook — con el cap modesto por defecto de
            // algunos proveedores trunca a mitad de tool-call
            // (`finish_reason=Length`, observado en producción).
            .max_tokens(max_tokens);
        let agent = match scope {
            DiagnosticToolScope::Assessment => base.tool(SubmitDiagnosticAssessmentTool(capture)).build(),
            DiagnosticToolScope::Battery => base.tool(PresentDiagnosticBatteryTool(capture)).build(),
            DiagnosticToolScope::Propose => {
                base.tool(ProposeSyllabusPlanTool(capture.clone())).tool(ProposeCapstoneProjectTool(capture)).build()
            }
            DiagnosticToolScope::All => base
                .tool(SubmitDiagnosticAssessmentTool(capture.clone()))
                .tool(PresentDiagnosticBatteryTool(capture.clone()))
                .tool(ProposeSyllabusPlanTool(capture.clone()))
                .tool(ProposeCapstoneProjectTool(capture.clone()))
                .tool(ConfirmSyllabusPlanTool(capture))
                .build(),
        };
        // Rig's implicit budget is ONE model call — enough for a single tool
        // call at most. This agent may chain up to 2 SEQUENTIAL tool calls in
        // one turn (submit_diagnostic_assessment + present_diagnostic_battery,
        // propose_syllabus_plan + propose_capstone_project, OR — for the
        // absolute_zero skip path — submit_diagnostic_assessment +
        // propose_syllabus_plan + propose_capstone_project, 3 in a row —
        // confirm_syllabus_plan is never chained with any of the above, see
        // `RoadmapCapture`'s doc comment) — each consumes at least one turn,
        // since the model sees each result before deciding the next call —
        // plus a final turn, or Rig aborts with `MaxTurnsError` before the
        // last call ever happens.
        const DIAGNOSTIC_MAX_TURNS: usize = 6;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS),
            agent.prompt(input).max_turns(DIAGNOSTIC_MAX_TURNS),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s")))?
        .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput {
            text,
            tool_calls: vec!["submit_diagnostic_assessment".to_string(), "generate_syllabus_execution".to_string()],
        })
    }

    /// Build a Rig agent with the single `publish_notebook_block` tool
    /// registered and run one turn — produces ONE block, not a whole
    /// notebook. Used only by `notebook_service::generation`.
    pub async fn run_notebook_block_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: NotebookBlockCapture,
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_notebook_block_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider)).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_notebook_block_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider)).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_notebook_block_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider)).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_notebook_block_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider)).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_notebook_block_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider)).await
            }
        }
    }

    async fn prompt_with_notebook_block_tool<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: NotebookBlockCapture,
        max_tokens: u64,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // A single block can still be a rich visual spec (mermaid/SVG groups,
        // walkthrough steps): the budget comes from the provider (see
        // `output_budget`), not the modest plain-turn default — same as the
        // retired whole-notebook call, just spent on one block instead of 3-5.
        let agent = AgentBuilder::new(model).preamble(system_prompt).max_tokens(max_tokens).tool(PublishNotebookBlockTool(capture)).build();
        // A single tool call still sometimes needs a follow-up turn (provider
        // dependent) — same generous budget as the roadmap execution turn.
        const NOTEBOOK_BLOCK_MAX_TURNS: usize = 6;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS),
            agent.prompt(input).max_turns(NOTEBOOK_BLOCK_MAX_TURNS),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s")))?
        .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["publish_notebook_block".to_string()] })
    }

    /// Build a Rig agent with the single `grade_gate_submission` tool
    /// registered and run one turn. Used only by `notebook_service::grading`
    /// for the two free-text-graded block types (`heuristic_error_audit`,
    /// `hands_on_mission`) — deterministic gates never reach this path.
    pub async fn run_gate_grading_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: GateGradingCapture,
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_gate_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_gate_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_gate_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_gate_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_gate_grading_tool(model, system_prompt, input, capture).await
            }
        }
    }

    async fn prompt_with_gate_grading_tool<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: GateGradingCapture,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // A grading verdict + one short scaffold hint is tiny compared to a
        // notebook block — the modest plain-turn budget is plenty, no need
        // for the per-provider `output_budget`.
        let agent =
            AgentBuilder::new(model).preamble(system_prompt).max_tokens(PLAIN_MAX_TOKENS).tool(GradeGateSubmissionTool(capture)).build();
        const GATE_GRADING_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS),
            agent.prompt(input).max_turns(GATE_GRADING_MAX_TURNS),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s")))?
        .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["grade_gate_submission".to_string()] })
    }

    /// Build a Rig agent with the single `grade_closure_submission` tool
    /// registered and run one turn. Used only by `notebook_service::grading`
    /// for the closing block's student reflection — its pass is what
    /// completes the class (see `domain::notebook::class_is_complete`).
    pub async fn run_closure_grading_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: ClosureFeedbackCapture,
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_closure_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_closure_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_closure_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_closure_grading_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_closure_grading_tool(model, system_prompt, input, capture).await
            }
        }
    }

    async fn prompt_with_closure_grading_tool<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: ClosureFeedbackCapture,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // A verdict + one short student-facing feedback is tiny — the same
        // modest plain-turn budget as gate grading is plenty.
        let agent = AgentBuilder::new(model)
            .preamble(system_prompt)
            .max_tokens(PLAIN_MAX_TOKENS)
            .tool(GradeClosureSubmissionTool(capture))
            .build();
        const CLOSURE_GRADING_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS),
            agent.prompt(input).max_turns(CLOSURE_GRADING_MAX_TURNS),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s")))?
        .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["grade_closure_submission".to_string()] })
    }

    pub async fn run_block_audit_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: BlockAuditCapture,
    ) -> AppResult<PromptOutput> {
        Self::require_key(api_key, &config.id)?;
        let model_name = Self::resolve_model(config, None)?;
        let base_url = config.effective_base_url();

        match config.provider {
            ProviderKind::OpenAI => {
                let client = rig_core::providers::openai::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_block_audit_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_block_audit_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_block_audit_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_block_audit_tool(model, system_prompt, input, capture).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_block_audit_tool(model, system_prompt, input, capture).await
            }
        }
    }

    async fn prompt_with_block_audit_tool<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: BlockAuditCapture,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // One verdict + a short feedback list — same plain-turn budget as the
        // gate grader, no per-provider output_budget needed.
        let agent = AgentBuilder::new(model).preamble(system_prompt).max_tokens(PLAIN_MAX_TOKENS).tool(SubmitBlockAuditTool(capture)).build();
        const BLOCK_AUDIT_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(PROMPT_TIMEOUT_SECS),
            agent.prompt(input).max_turns(BLOCK_AUDIT_MAX_TURNS),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {PROMPT_TIMEOUT_SECS}s")))?
        .map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["submit_block_audit".to_string()] })
    }

    /// Lightweight connectivity check (no tokens spent, no prompt sent).
    pub async fn verify_connection(config: &ProviderConfig, api_key: &str) -> AppResult<()> {
        Self::require_key(api_key, &config.id)?;
        let http = http_client();
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
        let http = http_client();
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

/// Maps a Rig prompt failure to a UI-safe error. A `finish_reason=Length`
/// truncation (the model ran out of output budget mid-answer) gets a plain
/// Spanish hint instead of raw provider English — retrying the same input
/// usually reproduces it, so the message says what actually helps.
fn agent_error(raw: &impl std::fmt::Display) -> AppError {
    let msg = raw.to_string();
    if msg.contains("finish_reason=Length") {
        return AppError::AgentExecutionFailed(
            "el modelo se quedó sin espacio de respuesta a mitad del turno \
             (límite de tokens de salida). Prueba con un modelo con mayor límite \
             o responde en dos mensajes más cortos."
                .to_string(),
        );
    }
    AppError::AgentExecutionFailed(trim_error(&msg))
}

/// Client-build failures go through the same redaction: a builder error can
/// echo the base URL (and, in theory, key material), so it never travels raw.
fn connect_error(raw: &impl std::fmt::Display) -> AppError {
    AppError::ProviderConnectionFailed(format!("build client: {}", trim_error(&raw.to_string())))
}

#[cfg(test)]
mod tests;
