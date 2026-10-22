//! Terminal-tool hook: ends a Rig tool loop as soon as the flow's capture is
//! complete, instead of letting the model burn extra turns (and seconds)
//! narrating or re-calling the tool until `MaxTurnsError`.

use std::sync::{Arc, Mutex};

use rig_agent::agent::{AgentHook, HookContext, ModelTurnAction, ModelTurnFinished, ToolCall, ToolCallAction, ToolResultAction, ToolResultEvent};
use rig_agent::completion::PromptError;
use rig_core::completion::Usage;

use crate::tools::roadmap_tools::{DiagnosticToolScope, RoadmapCapture};

/// Reason carried by our own `ToolResultAction::Stop`; the factory maps a
/// `PromptCancelled` with exactly this reason to success.
pub const STOP_REASON: &str = "learnkit:terminal-tool-captured";

/// Stable event name logged when a repeated tool call is skipped.
const TRUNCATED_EVENT: &str = "multiple_tool_calls_truncated";

/// Feedback returned to the model when it repeats an already-captured tool.
const ALREADY_CAPTURED_FEEDBACK: &str = "ya registrado; no repitas la herramienta";

/// Stable event name for the per-turn token usage record.
const USAGE_EVENT: &str = "model_turn_usage";

/// Dev-trace note for one model turn's provider-reported usage. Pure so the
/// format is testable without a model.
pub fn usage_notes(turn: usize, usage: &Usage) -> Vec<String> {
    vec![
        format!("turn={turn}"),
        format!("input_tokens={}", usage.input_tokens),
        format!("output_tokens={}", usage.output_tokens),
        format!("cached_input_tokens={}", usage.cached_input_tokens),
        format!("cache_creation_input_tokens={}", usage.cache_creation_input_tokens),
        format!("reasoning_tokens={}", usage.reasoning_tokens),
    ]
}

/// Pure decision for `on_tool_call`: the first valid call wins. A call is only
/// skipped once the capture is complete, so a corrected retry after an
/// invalid first call still runs.
pub fn tool_call_action(complete: bool) -> ToolCallAction {
    if complete { ToolCallAction::Skip(ALREADY_CAPTURED_FEEDBACK.to_string()) } else { ToolCallAction::Run }
}

/// Pure decision for `on_tool_result`: stop the loop once the capture is complete.
pub fn tool_result_action(complete: bool) -> ToolResultAction {
    if complete { ToolResultAction::stop(STOP_REASON) } else { ToolResultAction::Keep }
}

/// Whether the roadmap capture holds everything the given scope's turn needs.
///
/// `All` is deliberately conservative: that scope also serves chained
/// hand-offs (assessment + battery, assessment + propose + capstone on the
/// absolute_zero path), so it only stops on `confirm_syllabus_plan`, on
/// syllabus AND capstone together, or on the battery.
pub fn roadmap_capture_complete(scope: DiagnosticToolScope, capture: &RoadmapCapture) -> bool {
    let proposed = capture.propose_syllabus_plan.is_some() && capture.propose_capstone_project.is_some();
    match scope {
        DiagnosticToolScope::Assessment => capture.assessment.is_some(),
        DiagnosticToolScope::Battery => capture.diagnostic_battery.is_some(),
        DiagnosticToolScope::Propose => proposed,
        DiagnosticToolScope::All => capture.confirm_syllabus_plan.is_some() || proposed || capture.diagnostic_battery.is_some(),
    }
}

/// Stops the tool loop once `is_complete` reports the capture is filled, and
/// skips repeated calls after that (first call wins).
pub struct StopWhenCaptured<F>(F);

impl<F> StopWhenCaptured<F>
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    pub fn new(is_complete: F) -> Self {
        Self(is_complete)
    }
}

impl<F> AgentHook for StopWhenCaptured<F>
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    async fn on_tool_call(&self, _ctx: &HookContext, event: ToolCall<'_>) -> ToolCallAction {
        let action = tool_call_action((self.0)());
        if matches!(action, ToolCallAction::Skip(_)) {
            tracing::warn!(event = TRUNCATED_EVENT, tool = event.tool_name, "repeated tool call skipped: capture already complete");
            crate::dev_trace::trace_note("agent", "tool_loop", event.tool_name, TRUNCATED_EVENT, &[]);
        }
        action
    }

    async fn on_tool_result(&self, _ctx: &HookContext, _event: ToolResultEvent<'_>) -> ToolResultAction {
        tool_result_action((self.0)())
    }

    /// Observation only: records what the provider billed for this turn so
    /// prefix-size changes can be checked against real numbers.
    async fn on_model_turn_finished(&self, _ctx: &HookContext, event: ModelTurnFinished<'_>) -> ModelTurnAction {
        let u = &event.usage;
        tracing::info!(
            event = USAGE_EVENT,
            turn = event.turn,
            input_tokens = u.input_tokens,
            output_tokens = u.output_tokens,
            cached_input_tokens = u.cached_input_tokens,
            reasoning_tokens = u.reasoning_tokens,
            "model turn finished"
        );
        crate::dev_trace::trace_note("usage", "tool_loop", "model_turn", USAGE_EVENT, &usage_notes(event.turn, u));
        ModelTurnAction::Continue
    }
}

/// Hook for a capture that is complete as soon as it holds a value.
pub fn stop_when_some<T: Send + 'static>(capture: Arc<Mutex<Option<T>>>) -> StopWhenCaptured<impl Fn() -> bool + Send + Sync + 'static> {
    StopWhenCaptured::new(move || capture.lock().unwrap_or_else(|e| e.into_inner()).is_some())
}

/// Hook for the roadmap flows, scope-aware (see [`roadmap_capture_complete`]).
pub fn stop_when_roadmap_captured(
    scope: DiagnosticToolScope,
    capture: Arc<Mutex<RoadmapCapture>>,
) -> StopWhenCaptured<impl Fn() -> bool + Send + Sync + 'static> {
    StopWhenCaptured::new(move || roadmap_capture_complete(scope, &capture.lock().unwrap_or_else(|e| e.into_inner())))
}

/// Maps the result of a hooked run: our own stop is success (empty text; the
/// callers read the capture, not the text); every other error is untouched.
pub fn accept_terminal_stop(result: Result<String, PromptError>) -> Result<String, PromptError> {
    match result {
        Err(PromptError::PromptCancelled { ref reason, .. }) if reason == STOP_REASON => Ok(String::new()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::DiagnosticBattery;
    use crate::domain::roadmap::{CapstoneProjectArgs, ConfirmSyllabusPlanArgs, DiagnosticAssessmentArgs, ProposeSyllabusPlanArgs};

    fn from_json<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).expect("valid fixture")
    }

    fn assessment() -> DiagnosticAssessmentArgs {
        from_json(serde_json::json!({
            "topic": "Rust", "targetGoal": "x", "entryLevel": "absolute_zero", "timeframeWeeks": 4, "weeklyCommitmentHours": 8.0
        }))
    }

    fn battery() -> DiagnosticBattery {
        from_json(serde_json::json!({ "goalAlignment": "x", "questions": [] }))
    }

    fn syllabus_plan() -> ProposeSyllabusPlanArgs {
        from_json(serde_json::json!({
            "coreFocus": "x", "identifiedNeeds": [], "learningStrategy": "x", "closingQuestion": "x",
            "syllabus": { "courseTitle": "x", "totalWeeks": 1, "paceHoursPerWeek": 8.0, "milestones": [] }
        }))
    }

    fn capstone() -> CapstoneProjectArgs {
        from_json(serde_json::json!({ "title": "x", "description": "x", "verifiableEvidence": "x" }))
    }

    fn cancelled(reason: &str) -> Result<String, PromptError> {
        Err(PromptError::PromptCancelled { chat_history: vec![], reason: reason.to_string() })
    }

    #[test]
    fn empty_roadmap_capture_is_incomplete_for_every_scope() {
        let c = RoadmapCapture::default();
        for scope in [DiagnosticToolScope::All, DiagnosticToolScope::Assessment, DiagnosticToolScope::Battery, DiagnosticToolScope::Propose] {
            assert!(!roadmap_capture_complete(scope, &c));
        }
    }

    #[test]
    fn assessment_and_battery_scopes_complete_on_their_own_tool() {
        let mut c = RoadmapCapture::default();
        c.assessment = Some(assessment());
        assert!(roadmap_capture_complete(DiagnosticToolScope::Assessment, &c));
        assert!(!roadmap_capture_complete(DiagnosticToolScope::Battery, &c));
        // `All` does not stop on the assessment alone: a battery or a
        // propose hand-off may still follow in the same run.
        assert!(!roadmap_capture_complete(DiagnosticToolScope::All, &c));

        let mut c = RoadmapCapture::default();
        c.diagnostic_battery = Some(battery());
        assert!(roadmap_capture_complete(DiagnosticToolScope::Battery, &c));
        assert!(roadmap_capture_complete(DiagnosticToolScope::All, &c));
    }

    #[test]
    fn propose_scope_needs_both_syllabus_and_capstone() {
        let mut c = RoadmapCapture::default();
        c.propose_syllabus_plan = Some(syllabus_plan());
        assert!(!roadmap_capture_complete(DiagnosticToolScope::Propose, &c));
        assert!(!roadmap_capture_complete(DiagnosticToolScope::All, &c));
        c.propose_capstone_project = Some(capstone());
        assert!(roadmap_capture_complete(DiagnosticToolScope::Propose, &c));
        assert!(roadmap_capture_complete(DiagnosticToolScope::All, &c));
    }

    #[test]
    fn all_scope_completes_on_confirm() {
        let mut c = RoadmapCapture::default();
        c.confirm_syllabus_plan = Some(ConfirmSyllabusPlanArgs {});
        assert!(roadmap_capture_complete(DiagnosticToolScope::All, &c));
        assert!(!roadmap_capture_complete(DiagnosticToolScope::Propose, &c));
    }

    #[test]
    fn hook_decisions_stop_after_capture_and_skip_repeats() {
        assert_eq!(tool_result_action(false), ToolResultAction::Keep);
        assert_eq!(tool_result_action(true), ToolResultAction::Stop(STOP_REASON.to_string()));
        assert_eq!(tool_call_action(false), ToolCallAction::Run);
        assert_eq!(tool_call_action(true), ToolCallAction::Skip("ya registrado; no repitas la herramienta".to_string()));
    }

    #[test]
    fn usage_notes_carry_every_billed_counter() {
        let mut u = Usage::new();
        u.input_tokens = 1200;
        u.output_tokens = 80;
        u.cached_input_tokens = 1000;
        u.cache_creation_input_tokens = 7;
        u.reasoning_tokens = 3;
        assert_eq!(
            usage_notes(2, &u),
            ["turn=2", "input_tokens=1200", "output_tokens=80", "cached_input_tokens=1000", "cache_creation_input_tokens=7", "reasoning_tokens=3"]
        );
    }

    #[test]
    fn stop_when_some_tracks_the_capture() {
        let capture: Arc<Mutex<Option<u8>>> = Arc::new(Mutex::new(None));
        let hook = stop_when_some(capture.clone());
        assert!(!(hook.0)());
        *capture.lock().unwrap() = Some(1);
        assert!((hook.0)());
    }

    #[test]
    fn only_our_stop_reason_maps_to_success() {
        assert_eq!(accept_terminal_stop(cancelled(STOP_REASON)).unwrap(), "");
        assert!(matches!(accept_terminal_stop(cancelled("operator stop")), Err(PromptError::PromptCancelled { .. })));
        assert_eq!(accept_terminal_stop(Ok("hola".to_string())).unwrap(), "hola");
        let max = Err(PromptError::MaxTurnsError { max_turns: 6, chat_history: Box::new(vec![]), prompt: Box::new("x".into()) });
        assert!(matches!(accept_terminal_stop(max), Err(PromptError::MaxTurnsError { .. })));
    }

    /// Drives the REAL rig loop (as `providers/factory.rs` does) over a scripted
    /// model, so the "one model call, no MaxTurnsError" claim is observed rather
    /// than inferred from the pure decision functions.
    mod rig_loop {
        use super::*;
        use crate::tools::{BlockAuditCapture, BlockAuditResult, NotebookBlockCapture, PublishNotebookBlockTool, PublishedBlock, SubmitBlockAuditTool};
        use rig_agent::agent::AgentBuilder;
        use rig_agent::completion::Prompt;
        use rig_core::message::{AssistantContent, ToolCall, ToolFunction};
        use rig_core::test_utils::{MockCompletionModel, MockTurn};

        const MAX_TURNS: usize = 6;

        fn theory(title: &str) -> serde_json::Value {
            serde_json::json!({
                "blockType": "anchored_micro_theory", "title": title, "intuitiveHook": "h", "systemRule": "r", "frequentError": "f"
            })
        }

        fn true_false() -> serde_json::Value {
            serde_json::json!({ "blockType": "tpl_true_false", "statement": "s", "isTrue": true, "explanation": "e" })
        }

        fn call(id: &str, name: &str, args: serde_json::Value) -> AssistantContent {
            AssistantContent::ToolCall(ToolCall::from_wire(id, ToolFunction::new(name.to_string(), args)))
        }

        /// The `lead` turns, then enough identical repeats to outlast `MAX_TURNS`:
        /// a model that would keep re-calling the tool if the loop let it.
        fn script(lead: Vec<MockTurn>, repeat: impl Fn(usize) -> MockTurn) -> MockCompletionModel {
            let tail = (0..MAX_TURNS + 2).map(repeat);
            MockCompletionModel::from_turns(lead.into_iter().chain(tail))
        }

        async fn run_notebook(model: MockCompletionModel, capture: NotebookBlockCapture) -> Result<String, PromptError> {
            let hook = stop_when_some(capture.clone());
            let agent = AgentBuilder::new(model).tool(PublishNotebookBlockTool(capture)).build();
            accept_terminal_stop(agent.prompt("go").max_turns(MAX_TURNS).add_hook(hook).await)
        }

        async fn run_audit(model: MockCompletionModel, capture: BlockAuditCapture) -> Result<String, PromptError> {
            let hook = stop_when_some(capture.clone());
            let agent = AgentBuilder::new(model).tool(SubmitBlockAuditTool(capture)).build();
            accept_terminal_stop(agent.prompt("go").max_turns(MAX_TURNS).add_hook(hook).await)
        }

        fn block_json(captured: &Option<PublishedBlock>) -> serde_json::Value {
            serde_json::to_value(&captured.as_ref().expect("captured").block).expect("serializes")
        }

        #[tokio::test]
        async fn notebook_flow_ends_after_one_model_call_even_if_the_model_would_repeat() {
            let model = script(vec![], |_| MockTurn::tool_call("t", "publish_notebook_block", theory("A")));
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let text = run_notebook(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 1);
            assert_eq!(block_json(&capture.lock().unwrap())["title"], "A");
        }

        #[tokio::test]
        async fn notebook_flow_keeps_only_the_first_of_two_tool_calls_in_one_message() {
            let both = MockTurn::from_contents([call("a", "publish_notebook_block", theory("A")), call("b", "publish_notebook_block", true_false())]);
            let model = script(vec![both], |_| MockTurn::tool_call("t", "publish_notebook_block", true_false()));
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let text = run_notebook(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 1);
            let v = block_json(&capture.lock().unwrap());
            assert_eq!(v["blockType"], "anchored_micro_theory");
            assert_eq!(v["title"], "A");
        }

        #[tokio::test]
        async fn notebook_flow_does_not_skip_a_corrected_retry_after_an_invalid_call() {
            let invalid = MockTurn::tool_call("bad", "publish_notebook_block", serde_json::json!({ "blockType": "tpl_nope" }));
            let fixed = MockTurn::tool_call("ok", "publish_notebook_block", theory("fixed"));
            let model = script(vec![invalid, fixed], |_| MockTurn::tool_call("t", "publish_notebook_block", true_false()));
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let text = run_notebook(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 2);
            assert_eq!(block_json(&capture.lock().unwrap())["title"], "fixed");
        }

        #[tokio::test]
        async fn critic_flow_ends_after_one_model_call_even_if_the_model_would_repeat() {
            let rejected = serde_json::json!({ "accepted": false, "feedback": ["first"] });
            let model = script(vec![], |_| MockTurn::tool_call("t", "submit_block_audit", rejected.clone()));
            let capture: BlockAuditCapture = Arc::new(Mutex::new(None));
            let text = run_audit(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 1);
            let stored = capture.lock().unwrap();
            let audit: &BlockAuditResult = stored.as_ref().unwrap();
            assert!(!audit.accepted);
            assert_eq!(audit.feedback, vec!["first".to_string()]);
        }

        #[tokio::test]
        async fn critic_flow_keeps_only_the_first_of_two_tool_calls_in_one_message() {
            let both = MockTurn::from_contents([
                call("a", "submit_block_audit", serde_json::json!({ "accepted": false, "feedback": ["first"] })),
                call("b", "submit_block_audit", serde_json::json!({ "accepted": true, "feedback": [] })),
            ]);
            let model = script(vec![both], |_| MockTurn::tool_call("t", "submit_block_audit", serde_json::json!({ "accepted": true, "feedback": [] })));
            let capture: BlockAuditCapture = Arc::new(Mutex::new(None));
            let text = run_audit(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 1);
            assert!(!capture.lock().unwrap().as_ref().unwrap().accepted);
        }

        #[tokio::test]
        async fn critic_flow_does_not_skip_a_corrected_retry_after_an_invalid_call() {
            let invalid = MockTurn::tool_call("bad", "submit_block_audit", serde_json::json!({ "accepted": "maybe" }));
            let fixed = MockTurn::tool_call("ok", "submit_block_audit", serde_json::json!({ "accepted": true, "feedback": [] }));
            let model = script(vec![invalid, fixed], |_| MockTurn::tool_call("t", "submit_block_audit", serde_json::json!({ "accepted": false, "feedback": ["late"] })));
            let capture: BlockAuditCapture = Arc::new(Mutex::new(None));
            let text = run_audit(model.clone(), capture.clone()).await.expect("no MaxTurnsError");
            assert_eq!(text, "");
            assert_eq!(model.request_count(), 2);
            assert!(capture.lock().unwrap().as_ref().unwrap().accepted);
        }
    }
}
