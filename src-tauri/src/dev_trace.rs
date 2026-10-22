//! Developer-mode tracing: an in-memory ring buffer of what the generation
//! pipeline actually did (prompts, tool calls, critic verdicts, retries,
//! model errors), streamed to the UI as `dev://trace`.
//!
//! OFF by default and zero-cost while off: every `trace_*` entry point checks
//! [`is_enabled`] first, so no prompt/args are cloned or serialized unless the
//! developer opted in. Traces live in memory only (never disk, never
//! `tracing`), and secrets are redacted even here.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::error::AppError;

const CAPACITY: usize = 300;
/// Per-field cap so one 40 KB system prompt can't evict the whole buffer.
const MAX_FIELD_CHARS: usize = 60_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEntry {
    pub id: u64,
    pub ts_ms: i64,
    /// `agent_run | tool_call | retry | critique | loop_result | roadmap`
    pub kind: String,
    /// Which pipeline stage: `notebook_block`, `pedagogical_critic`, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    /// Class/document the entry belongs to, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_args: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u8>,
    /// Retry cause / critique verdict / loop source (`model` | `backstop`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transient: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_exhausted: Option<bool>,
}

impl TraceEntry {
    pub fn new(kind: &str) -> Self {
        Self { kind: kind.to_string(), ..Default::default() }
    }
}

/// Ring buffer + enable flag. A struct (not just statics) so tests can use an
/// isolated instance.
pub struct DevTrace {
    enabled: AtomicBool,
    next_id: AtomicU64,
    capacity: usize,
    buf: Mutex<VecDeque<TraceEntry>>,
    app: Mutex<Option<tauri::AppHandle>>,
}

impl DevTrace {
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            capacity: capacity.max(1),
            buf: Mutex::new(VecDeque::new()),
            app: Mutex::new(None),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    /// Stores (and, with an app handle, emits) the entry. No-op while off.
    pub fn record(&self, mut entry: TraceEntry) {
        if !self.is_enabled() {
            return;
        }
        entry.id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if entry.ts_ms == 0 {
            entry.ts_ms = now_ms();
        }
        scrub(&mut entry);
        {
            let mut buf = self.buf.lock().unwrap_or_else(|e| e.into_inner());
            if buf.len() >= self.capacity {
                buf.pop_front();
            }
            buf.push_back(entry.clone());
        }
        if let Some(app) = self.app.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = app.emit("dev://trace", &entry);
        }
    }

    pub fn snapshot(&self) -> Vec<TraceEntry> {
        self.buf.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.buf.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

fn global() -> &'static DevTrace {
    static TRACE: OnceLock<DevTrace> = OnceLock::new();
    TRACE.get_or_init(|| DevTrace::new(CAPACITY))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn is_enabled() -> bool {
    global().is_enabled()
}

pub fn set_enabled(on: bool) {
    global().set_enabled(on);
}

/// Called once from `lib.rs`'s setup hook (same pattern as the tool emitter).
pub fn set_app(app: tauri::AppHandle) {
    *global().app.lock().unwrap_or_else(|e| e.into_inner()) = Some(app);
}

pub fn record(entry: TraceEntry) {
    global().record(entry);
}

pub fn snapshot() -> Vec<TraceEntry> {
    global().snapshot()
}

pub fn clear() {
    global().clear();
}

/// Fills the error fields shared by every failing entry.
pub fn with_error(mut entry: TraceEntry, err: &AppError) -> TraceEntry {
    entry.ok = Some(false);
    entry.error_code = Some(err.code().to_string());
    entry.error = Some(err.to_string());
    entry.transient = Some(err.is_transient());
    entry.budget_exhausted = Some(err.is_output_budget_exhausted());
    entry
}

/// One tool invocation inside Rig's loop. `args` is whatever the model sent
/// (after the double-escape repair).
pub fn trace_tool<T: Serialize>(tool: &str, args: &T) {
    if !is_enabled() {
        return;
    }
    let mut entry = TraceEntry::new("tool_call");
    entry.tool = Some(tool.to_string());
    entry.tool_args = serde_json::to_value(args).ok();
    record(entry);
}

/// A pipeline decision worth seeing (grounding rejection, backstop used…).
pub fn trace_note(kind: &str, flow: &str, scope: &str, verdict: &str, notes: &[String]) {
    if !is_enabled() {
        return;
    }
    let mut entry = TraceEntry::new(kind);
    entry.flow = Some(flow.to_string());
    entry.scope = Some(scope.to_string());
    entry.verdict = Some(verdict.to_string());
    entry.notes = notes.to_vec();
    record(entry);
}

/// Redacts secrets and bounds every free-text field. Runs on every recorded
/// entry, so callers can't forget it.
fn scrub(entry: &mut TraceEntry) {
    for field in [&mut entry.system_prompt, &mut entry.input, &mut entry.output, &mut entry.error, &mut entry.verdict] {
        if let Some(s) = field.as_mut() {
            *s = bound(&redact(s));
        }
    }
    for n in entry.notes.iter_mut() {
        *n = bound(&redact(n));
    }
    if let Some(v) = entry.tool_args.as_mut() {
        redact_json(v);
    }
}

fn redact_json(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::String(s) => *s = bound(&redact(s)),
        serde_json::Value::Array(a) => a.iter_mut().for_each(redact_json),
        serde_json::Value::Object(o) => o.values_mut().for_each(redact_json),
        _ => {}
    }
}

fn bound(s: &str) -> String {
    if s.chars().count() <= MAX_FIELD_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_FIELD_CHARS).collect();
    out.push_str("\n…[truncado]");
    out
}

/// Replaces the secret that follows `sk-`, `Bearer ` or `key=` (up to the
/// next whitespace/quote) — unlike the one-line `trim_error`, the rest of the
/// text is kept so prompts stay readable.
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for marker in ["sk-", "Bearer ", "key="] {
        let mut from = 0;
        while let Some(rel) = out[from..].find(marker) {
            let start = from + rel + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '&' | ',' | ')' | '}'))
                .map(|i| start + i)
                .unwrap_or(out.len());
            // `sk-` as part of a word ("task-force") isn't a key: require a
            // plausible token length for it.
            if marker == "sk-" && end - start < 8 {
                from = start;
                continue;
            }
            out.replace_range(start..end, "[redacted]");
            from = start + "[redacted]".len();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(capacity: usize) -> DevTrace {
        let t = DevTrace::new(capacity);
        t.set_enabled(true);
        t
    }

    #[test]
    fn records_nothing_while_disabled() {
        let t = DevTrace::new(10);
        t.record(TraceEntry::new("agent_run"));
        assert!(t.snapshot().is_empty());
    }

    #[test]
    fn ring_buffer_drops_the_oldest_entries() {
        let t = on(3);
        for i in 0..5 {
            let mut e = TraceEntry::new("tool_call");
            e.tool = Some(format!("t{i}"));
            t.record(e);
        }
        let snap = t.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].tool.as_deref(), Some("t2"));
        assert_eq!(snap[2].id, 5, "ids keep increasing across evictions");
    }

    #[test]
    fn clear_empties_the_buffer() {
        let t = on(3);
        t.record(TraceEntry::new("retry"));
        t.clear();
        assert!(t.snapshot().is_empty());
    }

    #[test]
    fn redacts_keys_but_keeps_surrounding_text() {
        let s = redact("fallo con Bearer abc123xyz y sk-proj1234567890 en url?key=SECRET&a=1 task-force");
        assert!(!s.contains("abc123xyz"));
        assert!(!s.contains("proj1234567890"));
        assert!(!s.contains("SECRET"));
        assert!(s.contains("a=1"));
        assert!(s.contains("task-force"), "short `sk-` inside a word is not a key");
    }

    #[test]
    fn record_scrubs_every_text_field_and_tool_args() {
        let t = on(5);
        let mut e = TraceEntry::new("tool_call");
        e.input = Some("Bearer topsecretvalue".to_string());
        e.tool_args = Some(serde_json::json!({"a": ["key=hunter2"]}));
        t.record(e);
        let snap = t.snapshot();
        assert!(!snap[0].input.as_ref().unwrap().contains("topsecretvalue"));
        assert!(!snap[0].tool_args.as_ref().unwrap().to_string().contains("hunter2"));
    }

    #[test]
    fn fields_are_bounded() {
        let t = on(2);
        let mut e = TraceEntry::new("agent_run");
        e.system_prompt = Some("x".repeat(MAX_FIELD_CHARS + 500));
        t.record(e);
        let p = t.snapshot()[0].system_prompt.clone().unwrap();
        assert!(p.chars().count() < MAX_FIELD_CHARS + 50);
        assert!(p.ends_with("[truncado]"));
    }

    #[test]
    fn with_error_classifies_the_failure() {
        let e = with_error(TraceEntry::new("agent_run"), &AppError::AgentExecutionFailed("finish_reason=Length".to_string()));
        assert_eq!(e.ok, Some(false));
        assert_eq!(e.error_code.as_deref(), Some("AgentExecutionFailed"));
        assert_eq!(e.budget_exhausted, Some(true));
        assert_eq!(e.transient, Some(true));
    }

    #[test]
    fn serializes_camel_case_and_skips_empty_fields() {
        let mut e = TraceEntry::new("retry");
        e.duration_ms = Some(5);
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "retry");
        assert_eq!(v["durationMs"], 5);
        assert!(v.get("systemPrompt").is_none());
        assert!(v.get("notes").is_none());
    }
}
