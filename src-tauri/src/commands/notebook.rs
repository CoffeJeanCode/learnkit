use std::collections::HashMap;

use tauri::State;

use crate::domain::learner_memory::LearnerCognitiveMemory;
use crate::domain::notebook::{BlockUpdate, ClassRecord, ClosureFeedback, Course, DiagnosticBatteryState, GateResult, GateSubmission, NotebookPayload};
use crate::error::AppResult;
use crate::state::AppState;

/// Seeds `courses`/`syllabus_milestones`/`classes` in the SQLite store from
/// an already-sealed Roadmap & Syllabus Diagnostic session, returning the
/// classes the frontend can now request notebooks for (one per week).
///
/// Idempotent by construction: it delegates to
/// [`RoadmapService::ensure_course_imported`], which no-ops (and returns the
/// existing ids) when the session already has its course. The old
/// implementation imported unconditionally, so every call minted a NEW
/// duplicate course — the source of the "20 cursos, 8 con sesión" mess.
/// After the import it just lists the classes, so callers still get the
/// same `Vec<ClassRecord>` shape.
#[tauri::command]
pub fn import_course_from_roadmap(state: State<'_, AppState>, session_id: String) -> AppResult<Vec<ClassRecord>> {
    let session = state.roadmap_service.ensure_course_imported(&session_id)?;
    let course_id = session
        .imported_course_id
        .ok_or_else(|| crate::error::AppError::InvalidInput("session has no imported course".to_string()))?;
    state.notebook_service.list_course_classes(&course_id)
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

/// Regenerates ONE already-persisted block whose stored content the renderer
/// can't display anymore ("formato inesperado"), replacing it in place at the
/// same position and with the same block type. Returns the refreshed class so
/// the caller re-renders from fresh state. See
/// `NotebookService::regenerate_block`.
#[tauri::command]
pub async fn regenerate_notebook_block(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    block_id: String,
) -> AppResult<NotebookPayload> {
    state.notebook_service.regenerate_block(Some(&app), &block_id).await
}

/// Wipes an atypical/broken class's notebook entirely (see
/// `NotebookService::reset_class_notebook`) — the frontend follows this with
/// `start_class_notebook` to regenerate it from scratch, exactly the same
/// call it makes when opening a class for the first time.
#[tauri::command]
pub fn reset_class_notebook(state: State<'_, AppState>, class_id: String) -> AppResult<()> {
    state.notebook_service.reset_class_notebook(&class_id)
}

/// The student's self-report on ONE spaced-retrieval prompt, sent after they
/// saw the answer. The only path that moves retrieval mastery.
#[tauri::command]
pub fn record_retrieval_result(state: State<'_, AppState>, block_id: String, item_index: usize, recalled: bool) -> AppResult<()> {
    state.notebook_service.record_retrieval_result(&block_id, item_index, recalled)
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

/// Read-only: the `"local"` learner's cognitive profile — see
/// `domain::learner_memory`'s module doc for why `"local"` is the only
/// learner id this single-learner app ever uses.
#[tauri::command]
pub fn get_learner_memory(state: State<'_, AppState>) -> AppResult<LearnerCognitiveMemory> {
    state.notebook_service.get_learner_memory()
}
