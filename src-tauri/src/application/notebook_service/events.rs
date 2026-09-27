//! Emits `notebook_block://*` events during background generation (see
//! `generation::run_background_chain`) — the per-block counterpart to
//! `orchestration::orchestrator`'s per-agent-run `agent://*` events. Kept as
//! its own tiny module so `generation.rs` doesn't have to know about
//! `tauri::Emitter` directly.

use tauri::{AppHandle, Emitter};

use crate::domain::message::NotebookBlockEvent;
use crate::domain::notebook::NotebookBlock;

pub(super) fn emit_block_generating(app: Option<&AppHandle>, document_id: &str) {
    emit(app, "notebook_block://generating", NotebookBlockEvent::generating(document_id));
}

pub(super) fn emit_block_ready(app: Option<&AppHandle>, document_id: &str, block: &NotebookBlock) {
    emit(app, "notebook_block://ready", NotebookBlockEvent::ready(document_id, block.clone()));
}

pub(super) fn emit_block_error(app: Option<&AppHandle>, document_id: &str, message: &str) {
    emit(app, "notebook_block://error", NotebookBlockEvent::error(document_id, message));
}

fn emit(app: Option<&AppHandle>, channel: &str, event: NotebookBlockEvent) {
    if let Some(handle) = app {
        // Event delivery must never fail a run — best-effort, same as the
        // orchestrator's own `agent://*` emits.
        let _ = handle.emit(channel, &event);
    }
}
