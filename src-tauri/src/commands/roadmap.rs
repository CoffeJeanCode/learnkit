use serde::Serialize;
use tauri::State;

use crate::domain::roadmap::{RoadmapSession, RoadmapSessionSummary};
use crate::error::AppResult;
use crate::state::AppState;

/// One turn's response: the conversational text plus the updated session so
/// the UI can render phase/progress without a second round-trip.
#[derive(Debug, Serialize)]
pub struct RoadmapTurnPayload {
    pub message: String,
    pub session: RoadmapSession,
}

#[tauri::command]
pub async fn start_roadmap_session(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.start_session(Some(&app)).await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

#[tauri::command]
pub async fn send_roadmap_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.send_message(Some(&app), &session_id, &message).await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Re-runs the turn that just failed (network blip / timeout) after the
/// backend's own retry budget ran out — backs the UI's manual "Reintentar"
/// and its one automatic re-attempt. Doesn't replay the student's input
/// (a failed turn already persisted it), so the log never duplicates.
#[tauri::command]
pub async fn retry_roadmap_turn(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.retry_pending_turn(Some(&app), &session_id).await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

#[tauri::command]
pub fn get_roadmap_session(state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapSession> {
    state.roadmap_service.get_session(&session_id)
}

#[tauri::command]
pub fn list_roadmap_sessions(state: State<'_, AppState>) -> AppResult<Vec<RoadmapSessionSummary>> {
    state.roadmap_service.list_sessions()
}

#[tauri::command]
pub fn rename_roadmap_session(
    state: State<'_, AppState>,
    session_id: String,
    title: String,
) -> AppResult<RoadmapSessionSummary> {
    state.roadmap_service.rename_session(&session_id, &title)
}

#[tauri::command]
pub fn delete_roadmap_session(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.roadmap_service.delete_session(&session_id)
}

/// Idempotently ensures this sealed session's course/classes exist in the
/// notebook store, returning the (possibly updated) session. Recovery path
/// for a session sealed before that import existed — the frontend calls
/// this whenever `imported_course_id`/`first_class_id` are missing, instead
/// of a "Ver tu primera clase" click doing nothing.
#[tauri::command]
pub fn ensure_course_imported(state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapSession> {
    state.roadmap_service.ensure_course_imported(&session_id)
}

/// Records one answer to the session's pending diagnostic battery. Once the
/// last question is answered this also runs the Roadmap-stage turn and
/// returns an (often now-sealed) session in the same response — no separate
/// call needed to trigger it.
#[tauri::command]
pub async fn answer_diagnostic_question(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    question_index: usize,
    answer: String,
) -> AppResult<RoadmapTurnPayload> {
    let result = state
        .roadmap_service
        .answer_diagnostic_question(Some(&app), &session_id, question_index, &answer)
        .await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Escape hatch: finishes the Diagnostic stage with whatever's answered so
/// far, so a student is never stuck on the battery with no way forward.
#[tauri::command]
pub async fn skip_diagnostic_battery(app: tauri::AppHandle, state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.skip_diagnostic_battery(Some(&app), &session_id).await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}
