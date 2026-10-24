use serde::Serialize;
use tauri::State;

use crate::domain::learning_context::ContextAnswer;
use crate::domain::roadmap::{PlanAdjustment, ProfileBasics, RoadmapSession, RoadmapSessionSummary};
use crate::error::AppResult;
use crate::state::AppState;

/// Starts the resource research as soon as a command leaves the session
/// SEALED (its course now exists). Called after every command that can seal;
/// idempotent — the research claims itself, so a repeat costs one cheap read.
fn kick_research(app: &tauri::AppHandle, state: &AppState, session: &RoadmapSession) {
    use crate::domain::roadmap::SessionStatus;
    if session.status == SessionStatus::Sealed {
        if let Some(course_id) = &session.imported_course_id {
            state.research_service.start(Some(app.clone()), course_id.clone());
        }
    }
}

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
    kick_research(&app, &state, &result.session);
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Step 1 -> 2 of the profile form: validates topic, goal, weeks and hours and
/// returns the session with intake questions generated for THIS topic (one fast
/// model call; a generic set if the model cannot produce a valid one).
#[tauri::command]
pub async fn propose_context_questions(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    basics: ProfileBasics,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.propose_context_questions(Some(&app), &session_id, basics).await?;
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Step 2 of the profile form (replaces the analyst's chat turns): validates the
/// answers against the questions the server kept, deduces the starting level,
/// saves the profile and runs the hand-off behind it (the battery, or the
/// proposal for an absolute beginner).
#[tauri::command]
pub async fn submit_roadmap_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    answers: Vec<ContextAnswer>,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.submit_profile(Some(&app), &session_id, answers).await?;
    kick_research(&app, &state, &result.session);
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Confirms the proposed plan with a button — seals the session without a
/// model call.
#[tauri::command]
pub fn confirm_roadmap_plan(app: tauri::AppHandle, state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.confirm_proposed_plan(&session_id)?;
    kick_research(&app, &state, &result.session);
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Revises the proposed plan from structured controls (weeks, hours, emphasis,
/// optional short note) — one proposal turn instead of a negotiation chat.
#[tauri::command]
pub async fn adjust_roadmap_plan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    adjustment: PlanAdjustment,
) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.adjust_proposed_plan(Some(&app), &session_id, adjustment).await?;
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
    kick_research(&app, &state, &result.session);
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
    kick_research(&app, &state, &result.session);
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}

/// Escape hatch: finishes the Diagnostic stage with whatever's answered so
/// far, so a student is never stuck on the battery with no way forward.
#[tauri::command]
pub async fn skip_diagnostic_battery(app: tauri::AppHandle, state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapTurnPayload> {
    let result = state.roadmap_service.skip_diagnostic_battery(Some(&app), &session_id).await?;
    kick_research(&app, &state, &result.session);
    Ok(RoadmapTurnPayload { message: result.message, session: result.session })
}
