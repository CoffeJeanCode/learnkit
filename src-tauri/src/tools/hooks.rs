//! Terminal-tool hook: ends a Rig tool loop as soon as the flow's capture is
//! complete, instead of letting the model burn extra turns (and seconds)
//! narrating or re-calling the tool until `MaxTurnsError`.

use std::sync::{Arc, Mutex};

use rig_agent::agent::{AgentHook, HookContext, ToolCall, ToolCallAction, ToolResultAction, ToolResultEvent};
use rig_agent::completion::PromptError;

use crate::tools::roadmap_tools::{DiagnosticToolScope, RoadmapCapture};

/// Reason carried by our own `ToolResultAction::Stop`; the factory maps a
/// `PromptCancelled` with exactly this reason to success.
pub const STOP_REASON: &str = "learnkit:terminal-tool-captured";

/// Feedback returned to the model when it repeats an already-captured tool.
const ALREADY_CAPTURED_FEEDBACK: &str = "ya registrado; no repitas la herramienta";

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
    async fn on_tool_call(&self, _ctx: &HookContext, _event: ToolCall<'_>) -> ToolCallAction {
        tool_call_action((self.0)())
    }

    async fn on_tool_result(&self, _ctx: &HookContext, _event: ToolResultEvent<'_>) -> ToolResultAction {
        tool_result_action((self.0)())
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
}
