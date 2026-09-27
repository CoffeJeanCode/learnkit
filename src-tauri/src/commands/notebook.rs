use std::collections::HashMap;

use tauri::State;

use crate::domain::notebook::{BlockUpdate, ClassRecord, ClosureFeedback, Course, DiagnosticBatteryState, GateResult, GateSubmission, NotebookPayload};
use crate::error::AppResult;
use crate::state::AppState;

/// Seeds `courses`/`syllabus_milestones`/`classes` in the SQLite store from
/// an already-sealed Roadmap & Syllabus Diagnostic session, returning the
/// classes the frontend can now request notebooks for (one per week).
#[tauri::command]
pub fn import_course_from_roadmap(state: State<'_, AppState>, session_id: String) -> AppResult<Vec<ClassRecord>> {
    let session = state.roadmap_service.get_session(&session_id)?;
    state.notebook_service.import_course_from_roadmap(&session)
}

#[tauri::command]
pub fn list_course_classes(state: State<'_, AppState>, course_id: String) -> AppResult<Vec<ClassRecord>> {
    state.notebook_service.list_course_classes(&course_id)
}

#[tauri::command]
pub fn list_courses(state: State<'_, AppState>) -> AppResult<Vec<Course>> {
    state.notebook_service.list_courses()
}

/// Starts (or resumes) a class's notebook: returns block 1 immediately —
/// generated synchronously — and buffers subsequent blocks in the
/// background (see `notebook_service::generation`), announced to the
/// frontend via `notebook_block://*` events. Calling this again on an
/// already-started class just returns current progress, never regenerates.
#[tauri::command]
pub async fn start_class_notebook(app: tauri::AppHandle, state: State<'_, AppState>, class_id: String) -> AppResult<NotebookPayload> {
    state.notebook_service.start_class_notebook(Some(&app), &class_id).await
}

/// Whatever's persisted so far for this class — the resume/reload path.
/// Never triggers generation itself.
#[tauri::command]
pub fn get_class_notebook_progress(state: State<'_, AppState>, class_id: String) -> AppResult<NotebookPayload> {
    state.notebook_service.get_class_notebook_progress(&class_id)
}

/// The literal mastery gate: grades `submission` against `block_id`'s
/// content and advances (or locks/escalates) the notebook accordingly. See
/// `domain::notebook::GateResult` for what each outcome carries back.
#[tauri::command]
pub async fn submit_gate_response(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    block_id: String,
    submission: GateSubmission,
) -> AppResult<GateResult> {
    state.notebook_service.submit_gate_response(Some(&app), &block_id, submission).await
}

/// Grades the closing block's reflection (`closure_feedback_grader`):
/// persists the submitted reflection, stores the verdict on the block
/// (status + `last_feedback`), and returns the ALWAYS student-facing
/// feedback. A `passed` verdict is what completes the class.
#[tauri::command]
pub async fn grade_closure_reflection(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    block_id: String,
    reflection: String,
) -> AppResult<ClosureFeedback> {
    state.notebook_service.submit_closure_feedback(Some(&app), &block_id, &reflection).await
}

/// Atomic retry of the pending (still-generating-or-failed) block for a
/// document — used by the frontend when a `notebook_block://error` event
/// fires. Never touches earlier persisted blocks.
#[tauri::command]
pub async fn retry_pending_block(app: tauri::AppHandle, state: State<'_, AppState>, document_id: String) -> AppResult<()> {
    state.notebook_service.retry_pending_block(Some(&app), &document_id).await
}

#[tauri::command]
pub fn save_notebook_state(state: State<'_, AppState>, notebook_id: String, blocks: Vec<BlockUpdate>) -> AppResult<()> {
    state.notebook_service.save_notebook_state(&notebook_id, &blocks)
}

/// The course's calibration battery (generated once, alongside its
/// syllabus — see `RoadmapService::seal_session`) plus whatever the student
/// has answered so far. `None` if this course predates the feature or its
/// syllabus generation didn't produce one.
#[tauri::command]
pub fn get_course_diagnostic_battery(state: State<'_, AppState>, course_id: String) -> AppResult<Option<DiagnosticBatteryState>> {
    state.notebook_service.get_course_diagnostic_battery(&course_id)
}

#[tauri::command]
pub fn save_diagnostic_battery_answers(
    state: State<'_, AppState>,
    course_id: String,
    answers: HashMap<String, String>,
) -> AppResult<()> {
    state.notebook_service.save_diagnostic_battery_answers(&course_id, &answers)
}
