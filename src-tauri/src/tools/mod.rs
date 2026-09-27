pub mod echo;
pub mod notebook_tools;
pub mod registry;
pub mod roadmap_tools;

use std::sync::OnceLock;
use tauri::Emitter;

use crate::domain::message::AgentEvent;

pub use echo::EchoTool;
pub use notebook_tools::{
    BlockAuditCapture, BlockAuditResult, ClosureFeedbackCapture, ClosureFeedbackResult, GateGradingCapture, GateGradingResult,
    GateScaffold, GradeClosureSubmissionTool, GradeGateSubmissionTool, NotebookBlockCapture, PublishNotebookBlockTool, ScaffoldType,
    SubmitBlockAuditTool,
};
pub use registry::{ToolInfo, is_known_tool, list_tools};
pub use roadmap_tools::{
    ConfirmSyllabusPlanTool, DiagnosticToolScope, PresentDiagnosticBatteryTool, ProposeSyllabusPlanTool, RoadmapCapture,
    SharedRoadmapCapture, SubmitDiagnosticAssessmentTool,
};

/// The app handle, registered once at setup. Tool implementations run deep
/// inside Rig's tool loop where no `AppHandle` is in scope, so they go
/// through this instead of threading one through every capture type.
static EMITTER: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Best-effort: like the orchestrator's own emits, delivery never fails a run.
pub fn emit_tool_event(tool: &str) {
    if let Some(app) = EMITTER.get() {
        let _ = app.emit("agent://tool", &AgentEvent::tool(tool));
    }
}

/// Called once from `lib.rs`'s setup hook.
pub fn set_tool_event_emitter(app: tauri::AppHandle) {
    let _ = EMITTER.set(app);
}
