//! Emits `notebook_block://*` events during background generation (see
//! `generation::run_background_chain`) — the per-block counterpart to
//! `orchestration::orchestrator`'s per-agent-run `agent://*` events. Kept as
//! its own tiny module so `generation.rs` doesn't have to know about
//! `tauri::Emitter` directly.

use tauri::{AppHandle, Emitter};

use crate::domain::message::NotebookBlockEvent;
use crate::domain::notebook::NotebookBlock;
use crate::tools::block_stream::BlockStreamUpdate;

pub(super) fn emit_block_generating(app: Option<&AppHandle>, document_id: &str) {
    emit(app, "notebook_block://generating", NotebookBlockEvent::generating(document_id));
}

pub(super) fn emit_block_ready(app: Option<&AppHandle>, document_id: &str, block: &NotebookBlock) {
    emit(app, "notebook_block://ready", NotebookBlockEvent::ready(document_id, block.clone()));
}

pub(super) fn emit_block_error(app: Option<&AppHandle>, document_id: &str, message: &str) {
    emit(app, "notebook_block://error", NotebookBlockEvent::error(document_id, message));
}

/// Live progress of the block being written (`tools::block_stream`): either a
/// sanitized preview or the amount of reasoning so far.
pub(super) fn emit_block_delta(app: Option<&AppHandle>, document_id: &str, attempt: u8, update: BlockStreamUpdate) {
    let event = match update {
        BlockStreamUpdate::Preview(preview) => NotebookBlockEvent::delta(document_id, attempt, Some(preview), None),
        BlockStreamUpdate::Reasoning { chars } => NotebookBlockEvent::delta(document_id, attempt, None, Some(chars)),
    };
    emit(app, "notebook_block://delta", event);
}

fn emit(app: Option<&AppHandle>, channel: &str, event: NotebookBlockEvent) {
    if let Some(handle) = app {
        // Event delivery must never fail a run — best-effort, same as the
        // orchestrator's own `agent://*` emits.
        let _ = handle.emit(channel, &event);
    }
}
