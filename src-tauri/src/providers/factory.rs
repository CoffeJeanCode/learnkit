use rig_agent::AgentBuilder;
use rig_agent::completion::Prompt;
use rig_core::client::CompletionClient;

use crate::domain::model::ModelInfo;
use crate::domain::provider::{ProviderConfig, ProviderKind};
use crate::error::{AppError, AppResult};
use crate::tools::block_stream::{BlockStreamHook, BlockStreamSink, current_sink};
use crate::tools::{
    BlockAuditCapture, ClosureFeedbackCapture, ConfirmSyllabusPlanTool, DiagnosticToolScope, EchoTool, GateGradingCapture,
    GradeClosureSubmissionTool, GradeGateSubmissionTool, NotebookBlockCapture, PresentDiagnosticBatteryTool,
    ProposeCapstoneProjectTool, ProposeSyllabusPlanTool, PublishNotebookBlockTool, SharedRoadmapCapture, SubmitBlockAuditTool,
    ResearchStep, SharedResearchCapture, StopWhenCaptured, SubmitContextQuestionsTool, SubmitDiagnosticAssessmentTool, SubmitPlanOutlineTool, SubmitResearchPlanTool, SubmitResourceSelectionTool, SubmitWeekSessionsTool, STOP_REASON, accept_terminal_stop, is_known_tool, stop_when_roadmap_captured, stop_when_some,
};

/// Pure parse of the `LEARNKIT_STREAM_BLOCKS` kill-switch: streaming is ON
/// unless it is explicitly `0`/`false`/`off`/`no`.
fn parse_stream_blocks(raw: Option<&str>) -> bool {
    !matches!(raw.map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("0" | "false" | "off" | "no"))
}

/// The progress sink for the block turn about to run, or `None` when nobody
/// scoped one or streaming is switched off (`LEARNKIT_STREAM_BLOCKS=0`).
fn block_streaming_sink() -> Option<BlockStreamSink> {
    if !parse_stream_blocks(std::env::var("LEARNKIT_STREAM_BLOCKS").ok().as_deref()) {
        return None;
    }
    current_sink()
}

/// Límite para una llamada al LLM: sin esto, una red colgada o un proveedor
/// lento deja el comando Tauri pendiente para siempre y la ventana parece
/// "no responder" (el composer queda deshabilitado en `sending` sin salida).
const DEFAULT_PROMPT_TIMEOUT_SECS: u64 = 200;
const MIN_PROMPT_TIMEOUT_SECS: u64 = 30;
const MAX_PROMPT_TIMEOUT_SECS: u64 = 900;

/// Pure parse/clamp of the `LEARNKIT_PROMPT_TIMEOUT_SECS` override: invalid or
/// missing values fall back to the app default, valid ones are clamped.
fn parse_prompt_timeout(raw: Option<&str>) -> u64 {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(MIN_PROMPT_TIMEOUT_SECS, MAX_PROMPT_TIMEOUT_SECS))
        .unwrap_or(DEFAULT_PROMPT_TIMEOUT_SECS)
}

/// Knobs for ONE notebook-block model call, in one place. Defaults are tuned
/// for a reasoning model (~5k thinking tokens + a multi-thousand-token tool
/// payload); each can be overridden from the environment without a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockCallParams {
    /// Model turns per call. One valid call ends the loop (stop hook), so extra
    /// turns only exist to fix a malformed call; each re-sends the whole context.
    max_turns: usize,
    /// Wall-clock ceiling for the call. `LEARNKIT_PROMPT_TIMEOUT_SECS`.
    timeout_secs: u64,
    /// Force a tool call instead of letting the model answer in prose.
    /// `LEARNKIT_NOTEBOOK_TOOL_CHOICE=auto` for APIs that reject `required`.
    require_tool: bool,
}

impl BlockCallParams {
    const DEFAULT_MAX_TURNS: usize = 3;
    const DEFAULT_TIMEOUT_SECS: u64 = 420;

    fn resolve(timeout: Option<&str>, turns: Option<&str>, tool_choice: Option<&str>) -> Self {
        Self {
            max_turns: turns
                .and_then(|v| v.trim().parse::<usize>().ok())
                .map(|v| v.clamp(1, 10))
                .unwrap_or(Self::DEFAULT_MAX_TURNS),
            timeout_secs: timeout
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|v| v.clamp(MIN_PROMPT_TIMEOUT_SECS, MAX_PROMPT_TIMEOUT_SECS))
                .unwrap_or(Self::DEFAULT_TIMEOUT_SECS),
            require_tool: tool_choice.map(str::trim) != Some("auto"),
        }
    }

    fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok();
        Self::resolve(
            var("LEARNKIT_PROMPT_TIMEOUT_SECS").as_deref(),
            var("LEARNKIT_NOTEBOOK_MAX_TURNS").as_deref(),
            var("LEARNKIT_NOTEBOOK_TOOL_CHOICE").as_deref(),
        )
    }

    fn tool_choice(&self) -> rig_core::completion::message::ToolChoice {
        use rig_core::completion::message::ToolChoice;
        if self.require_tool { ToolChoice::Required } else { ToolChoice::Auto }
    }
}

/// Effective per-call timeout; `LEARNKIT_PROMPT_TIMEOUT_SECS` lets slow
/// providers (e.g. live e2e runs) raise it without changing app defaults.
fn prompt_timeout_secs() -> u64 {
    parse_prompt_timeout(std::env::var("LEARNKIT_PROMPT_TIMEOUT_SECS").ok().as_deref())
}

/// The syllabus proposal writes the whole plan (weeks x sessions, each with five
/// fields) plus the capstone in ONE turn, and its size grows with the weeks and
/// weekly hours the learner asks for: 8 weeks at 12 h/week is 24 sessions. A
/// reasoning model needs minutes for that, so the default 200 s meant for short
/// answers timed out at exactly that step every time. Scopes that can emit the
/// proposal get [`PROPOSAL_TIMEOUT_FACTOR`] times the base limit (still
/// overridable through `LEARNKIT_PROMPT_TIMEOUT_SECS`), capped at
/// [`MAX_PROPOSAL_TIMEOUT_SECS`].
const PROPOSAL_TIMEOUT_FACTOR: u64 = 3;
const MAX_PROPOSAL_TIMEOUT_SECS: u64 = 1200;

fn proposal_timeout_secs(base: u64) -> u64 {
    base.saturating_mul(PROPOSAL_TIMEOUT_FACTOR).min(MAX_PROPOSAL_TIMEOUT_SECS)
}

fn diagnostic_timeout_secs(scope: DiagnosticToolScope) -> u64 {
    let base = prompt_timeout_secs();
    match scope {
        DiagnosticToolScope::Propose | DiagnosticToolScope::All => proposal_timeout_secs(base),
        DiagnosticToolScope::Assessment | DiagnosticToolScope::Battery => base,
    }
}
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

/// Anthropic prompt caching for the stable prefix (tools + system prompt).
///
/// Every flow re-sends a large, byte-identical prefix (generator: ~17k est.
/// tokens, roadmap: ~5-7k) on each attempt and on every tool-loop turn, and
/// across calls that are minutes apart. Two knobs, deliberately combined:
/// - `with_static_prefix_cache_ttl(OneHour)`: explicit markers on the final
///   tool definition and the system prompt. 1h (not the 5m default) because
///   roadmap turns are human-paced and a block batch can outlast 5 minutes;
///   the ~2x write cost is repaid by the first reuse of a prefix this size.
/// - `with_automatic_caching()`: Anthropic moves a single top-level breakpoint
///   over the conversation tail (5m), so turn N+1 of a tool loop reuses turn N.
///   We avoid `with_prompt_caching()` because its manual last-message marker
///   would be redundant with this and spend one of the 4 breakpoints.
/// Prefixes under the model minimum (1-4k tokens) are silently not cached by
/// the API, so the small graders are unaffected. Only the Anthropic arms call
/// this; other providers' request bodies are untouched.
fn anthropic_cached(model: rig_core::providers::anthropic::completion::CompletionModel) -> rig_core::providers::anthropic::completion::CompletionModel {
    model.with_automatic_caching().with_static_prefix_cache_ttl(rig_core::providers::anthropic::completion::CacheTtl::OneHour)
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
    /// pide 16384 donde el proveedor lo admite. DeepSeek y MiMo admiten
    /// mucho más (DeepSeek V4: hasta 384K según su doc oficial; MiMo:
    /// 131072): los modelos de razonamiento gastan salida pensando y con
    /// 8192 agotan el presupuesto (`finish_reason=Length`) antes de emitir
    /// la llamada a la herramienta. Deliberadamente por proveedor, nunca
    /// por nombre de modelo.
    pub fn output_budget(provider: &ProviderKind) -> u64 {
        match provider {
            ProviderKind::DeepSeek | ProviderKind::Xiaomi => 131_072,
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let text = tokio::time::timeout(std::time::Duration::from_secs(prompt_timeout_secs()), agent.prompt(input))
            .await
            .map_err(|_| {
                AppError::Timeout(format!("el proveedor no respondió en {}s", prompt_timeout_secs()))
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let hook = stop_when_roadmap_captured(scope, capture.clone());
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
        let timeout_secs = diagnostic_timeout_secs(scope);
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            agent.prompt(input).max_turns(DIAGNOSTIC_MAX_TURNS).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {timeout_secs}s")))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let hook = stop_when_some(capture.clone());
        let params = BlockCallParams::from_env();
        // Observed: the model sometimes answers in prose for every turn and never
        // calls the tool, so the call is forced (see `BlockCallParams`).
        let agent = AgentBuilder::new(model)
            .preamble(system_prompt)
            .max_tokens(max_tokens)
            .tool(PublishNotebookBlockTool(capture))
            .tool_choice(params.tool_choice())
            .build();
        let max_turns = params.max_turns;
        // Live progress: only when the caller scoped a sink around this attempt
        // (see `tools::block_stream`) and streaming is not switched off. Every
        // other caller — tests, mocks, no UI — keeps the blocking call below.
        if let Some(sink) = block_streaming_sink() {
            return Self::stream_notebook_block(&agent, input, hook, sink, params).await;
        }
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(params.timeout_secs),
            agent.prompt(input).max_turns(max_turns).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {}s", params.timeout_secs)))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["publish_notebook_block".to_string()] })
    }

    /// Runs one block-generation turn as a STREAM, feeding `sink` with
    /// sanitized progress (see `tools::block_stream`) while the model writes.
    /// Same stop-at-the-first-tool-result hook, same timeout, same error shape
    /// as the blocking path: the capture is filled by the tool exactly as
    /// before, so everything downstream (salvage, critic, persistence) is
    /// unchanged.
    async fn stream_notebook_block(
        agent: &rig_agent::agent::Agent,
        input: &str,
        stop: impl rig_agent::agent::AgentHook + 'static,
        sink: BlockStreamSink,
        params: BlockCallParams,
    ) -> AppResult<PromptOutput> {
        use futures::StreamExt;
        use rig_agent::completion::PromptError;
        use rig_agent::streaming::StreamingPrompt;

        let (timeout_secs, max_turns) = (params.timeout_secs, params.max_turns);
        let run = async {
            let mut stream = agent.stream_prompt(input).max_turns(max_turns).add_hook(stop).add_hook(BlockStreamHook::new(sink)).await;
            while let Some(item) = stream.next().await {
                item?;
            }
            Ok::<(), rig_agent::agent::StreamingError>(())
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), run)
            .await
            .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {timeout_secs}s")))?;
        match result {
            Ok(()) => {}
            // Our own terminal-tool stop is success, exactly as `accept_terminal_stop`
            // treats it on the blocking path.
            Err(rig_agent::agent::StreamingError::Prompt(e))
                if matches!(*e, PromptError::PromptCancelled { ref reason, .. } if reason == STOP_REASON) => {}
            Err(e) => return Err(AppError::AgentExecutionFailed(trim_error(&e.to_string()))),
        }
        Ok(PromptOutput { text: String::new(), tool_calls: vec!["publish_notebook_block".to_string()] })
    }

    /// Runs one step of the resource-research pipeline: the planner (propose
    /// queries) or the curator (pick among the hits found), each with its own
    /// single capture tool. Used only by `application::research_service`.
    pub async fn run_research_prompt(
        config: &ProviderConfig,
        api_key: &str,
        system_prompt: &str,
        input: &str,
        capture: SharedResearchCapture,
        step: ResearchStep,
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
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
            ProviderKind::Anthropic => {
                let client = rig_core::providers::anthropic::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = anthropic_cached(client.completion_model(model_name));
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
            ProviderKind::Gemini => {
                let client = rig_core::providers::gemini::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
            ProviderKind::OpenRouter => {
                let client = rig_core::providers::openrouter::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
            ProviderKind::DeepSeek => {
                let client = rig_core::providers::deepseek::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
                    .api_key(api_key.to_string())
                    .base_url(base_url)
                    .build()
                    .map_err(|e| connect_error(&e))?;
                let model = client.completion_model(model_name);
                Self::prompt_with_research_tool(model, system_prompt, input, capture, Self::output_budget(&config.provider), step).await
            }
        }
    }

    async fn prompt_with_research_tool<M>(
        model: M,
        system_prompt: &str,
        input: &str,
        capture: SharedResearchCapture,
        max_tokens: u64,
        step: ResearchStep,
    ) -> AppResult<PromptOutput>
    where
        M: rig_agent::completion::CompletionModel + 'static,
    {
        // Ends the loop as soon as THIS step's slot is filled, like every other
        // single-tool flow: no extra turns narrating or re-calling the tool.
        let stop_capture = capture.clone();
        let hook = StopWhenCaptured::new(move || stop_capture.lock().unwrap_or_else(|e| e.into_inner()).is_complete(step));
        let base = AgentBuilder::new(model).preamble(system_prompt).max_tokens(max_tokens);
        let agent = match step {
            ResearchStep::Plan => base.tool(SubmitResearchPlanTool(capture)).build(),
            ResearchStep::Select => base.tool(SubmitResourceSelectionTool(capture)).build(),
            ResearchStep::Questions => base.tool(SubmitContextQuestionsTool(capture)).build(),
            ResearchStep::Outline => base.tool(SubmitPlanOutlineTool(capture)).build(),
            ResearchStep::Week => base.tool(SubmitWeekSessionsTool(capture)).build(),
        };
        const RESEARCH_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(prompt_timeout_secs()),
            agent.prompt(input).max_turns(RESEARCH_MAX_TURNS).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {}s", prompt_timeout_secs())))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        let tool = match step {
            ResearchStep::Plan => "submit_research_plan",
            ResearchStep::Select => "submit_resource_selection",
            ResearchStep::Questions => "submit_context_questions",
            ResearchStep::Outline => "submit_plan_outline",
            ResearchStep::Week => "submit_week_sessions",
        };
        Ok(PromptOutput { text, tool_calls: vec![tool.to_string()] })
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let hook = stop_when_some(capture.clone());
        let agent =
            AgentBuilder::new(model).preamble(system_prompt).max_tokens(PLAIN_MAX_TOKENS).tool(GradeGateSubmissionTool(capture)).build();
        const GATE_GRADING_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(prompt_timeout_secs()),
            agent.prompt(input).max_turns(GATE_GRADING_MAX_TURNS).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {}s", prompt_timeout_secs())))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let hook = stop_when_some(capture.clone());
        let agent = AgentBuilder::new(model)
            .preamble(system_prompt)
            .max_tokens(PLAIN_MAX_TOKENS)
            .tool(GradeClosureSubmissionTool(capture))
            .build();
        const CLOSURE_GRADING_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(prompt_timeout_secs()),
            agent.prompt(input).max_turns(CLOSURE_GRADING_MAX_TURNS).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {}s", prompt_timeout_secs())))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
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
                let model = anthropic_cached(client.completion_model(model_name));
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
            ProviderKind::Xiaomi => {
                let client = rig_core::providers::xiaomimimo::Client::builder()
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
        let hook = stop_when_some(capture.clone());
        let agent = AgentBuilder::new(model).preamble(system_prompt).max_tokens(PLAIN_MAX_TOKENS).tool(SubmitBlockAuditTool(capture)).build();
        const BLOCK_AUDIT_MAX_TURNS: usize = 4;
        let text = tokio::time::timeout(
            std::time::Duration::from_secs(prompt_timeout_secs()),
            agent.prompt(input).max_turns(BLOCK_AUDIT_MAX_TURNS).add_hook(hook),
        )
        .await
        .map_err(|_| AppError::Timeout(format!("el proveedor no respondió en {}s", prompt_timeout_secs())))?;
        let text = accept_terminal_stop(text).map_err(|e| AppError::AgentExecutionFailed(trim_error(&e.to_string())))?;
        Ok(PromptOutput { text, tool_calls: vec!["submit_block_audit".to_string()] })
    }

    /// Lightweight connectivity check (no tokens spent, no prompt sent).
    pub async fn verify_connection(config: &ProviderConfig, api_key: &str) -> AppResult<()> {
        Self::require_key(api_key, &config.id)?;
        let http = http_client();
        let base_url = config.effective_base_url();

        let response = match config.provider {
            ProviderKind::OpenAI | ProviderKind::OpenRouter | ProviderKind::DeepSeek | ProviderKind::Xiaomi => {
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
            ProviderKind::OpenAI | ProviderKind::OpenRouter | ProviderKind::DeepSeek | ProviderKind::Xiaomi => {
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
