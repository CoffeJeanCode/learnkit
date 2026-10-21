use tauri::State;

use crate::dev_trace::{self, TraceEntry};
use crate::error::AppResult;
use crate::state::AppState;

/// Whether developer-mode tracing is on.
#[tauri::command]
pub fn get_dev_mode() -> bool {
    dev_trace::is_enabled()
}

/// Turns developer-mode tracing on/off (persisted). Turning it off also
/// drops what was captured, so prompts don't linger in memory.
#[tauri::command]
pub fn set_dev_mode(state: State<'_, AppState>, enabled: bool) -> AppResult<bool> {
    state.notebook_service.set_dev_mode_enabled(enabled)?;
    dev_trace::set_enabled(enabled);
    if !enabled {
        dev_trace::clear();
    }
    Ok(enabled)
}

/// Everything captured so far (oldest first), for the panel's initial load.
#[tauri::command]
pub fn get_dev_trace() -> Vec<TraceEntry> {
    dev_trace::snapshot()
}

#[tauri::command]
pub fn clear_dev_trace() {
    dev_trace::clear();
}
