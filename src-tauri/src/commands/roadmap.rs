use serde::Serialize;
use tauri::State;

use crate::domain::roadmap::RoadmapSession;
use crate::error::AppResult;
use crate::state::AppState;

/// One turn's response: the conversational text (the `roadmap_state` block
/// is stripped before it ever reaches the frontend) plus the updated session
/// so the UI can render phase/DoD progress without a second round-trip.
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

#[tauri::command]
pub fn get_roadmap_session(state: State<'_, AppState>, session_id: String) -> AppResult<RoadmapSession> {
    state.roadmap_service.get_session(&session_id)
}
