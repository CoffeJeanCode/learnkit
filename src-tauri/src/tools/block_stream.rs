//! Live progress of ONE block generation: turns the streamed argument
//! fragments of `publish_notebook_block` into a small, SAFE preview the UI can
//! show while the model is still writing.
//!
//! Safety is the point of the shape. The raw tool arguments carry answer keys
//! (`correctOption`, `isTrue`, a template's `explanation`, ...) that the
//! persisted block only hands to the client after `grading::redact_block`. The
//! stream never forwards raw arguments: [`preview_from_partial`] emits only the
//! block type and an explicit WHITELIST of reading-only prose fields
//! ([`PREVIEW_FIELDS`]). Every other block type streams as "type + progress"
//! with no content. Extending the whitelist is a deliberate, reviewed change.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rig_agent::agent::{AgentHook, HookContext, ObservationAction, ReasoningDelta, ToolCallDelta};
use serde_json::{Map, Value, json};

use super::partial_json::parse_partial;
use crate::domain::notebook::DynamicBlockType;

/// Minimum gap between two updates of the same kind: enough to feel live, few
/// enough not to flood the IPC channel with one event per token.
pub const MIN_EMIT_INTERVAL: Duration = Duration::from_millis(150);

/// Reading-only prose fields that may be shown while a block of that type is
/// being written, per block type. NEVER add a field that holds or hints at an
/// answer (options, correct answers, explanations of a gate, solutions).
const PREVIEW_FIELDS: &[(DynamicBlockType, &[&str])] =
    &[(DynamicBlockType::AnchoredMicroTheory, &["title", "intuitiveHook", "systemRule", "frequentError"])];

/// A preview field is cut at this many characters: it is a progress hint, not
/// the block.
const MAX_FIELD_CHARS: usize = 1500;

#[derive(Debug, Clone, PartialEq)]
pub enum BlockStreamUpdate {
    /// The model is still reasoning (before or between tool-call arguments);
    /// only the amount is reported, never the text.
    Reasoning { chars: usize },
    /// Sanitized partial block: `{ "blockType": string|null, "fields": {..} }`.
    Preview(Value),
}

pub type BlockStreamSink = Arc<dyn Fn(BlockStreamUpdate) + Send + Sync>;

tokio::task_local! {
    /// Where the generator's streamed progress goes. Scoped around ONE model
    /// attempt by the caller that knows the document (see
    /// `notebook_service::generation`); the provider factory reads it, so the
    /// `PromptRunner` seam and every mock stay untouched. Absent = no
    /// streaming (the factory then uses the plain blocking call).
    pub static BLOCK_STREAM_SINK: BlockStreamSink;
}

/// The sink in scope for the current task, if any.
pub fn current_sink() -> Option<BlockStreamSink> {
    BLOCK_STREAM_SINK.try_with(|s| s.clone()).ok()
}

/// Whether a previous update of this kind is old enough to send another.
fn due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|t| now.duration_since(t) >= MIN_EMIT_INTERVAL)
}

fn truncate_chars(s: &str) -> String {
    if s.chars().count() <= MAX_FIELD_CHARS { s.to_string() } else { s.chars().take(MAX_FIELD_CHARS).collect() }
}

/// Sanitizes a partially-parsed tool-call argument object into the preview the
/// client may see. The block type is forwarded only once it is a complete,
/// known type (a half-written `"anchored_mi"` is not one); content fields only
/// for types listed in [`PREVIEW_FIELDS`], and only string values.
pub fn preview_from_partial(partial: &Value) -> Value {
    let obj = partial.as_object();
    let block_type: Option<DynamicBlockType> =
        obj.and_then(|o| o.get("blockType")).and_then(Value::as_str).and_then(|s| serde_json::from_value(Value::String(s.to_string())).ok());

    let mut fields = Map::new();
    if let (Some(bt), Some(o)) = (block_type, obj) {
        if let Some((_, allowed)) = PREVIEW_FIELDS.iter().find(|(t, _)| *t == bt) {
            for key in *allowed {
                if let Some(text) = o.get(*key).and_then(Value::as_str) {
                    fields.insert((*key).to_string(), Value::String(truncate_chars(text)));
                }
            }
        }
    }
    json!({ "blockType": block_type.map(|b| b.as_str()), "fields": fields })
}

#[derive(Default)]
struct HookState {
    args: String,
    last_preview_emit: Option<Instant>,
    last_preview: Option<Value>,
    reasoning_chars: usize,
    last_reasoning_emit: Option<Instant>,
}

/// Observation-only hook feeding a [`BlockStreamSink`]. Never stops or alters
/// the run.
pub struct BlockStreamHook {
    sink: BlockStreamSink,
    state: Mutex<HookState>,
}

impl BlockStreamHook {
    pub fn new(sink: BlockStreamSink) -> Self {
        Self { sink, state: Mutex::new(HookState::default()) }
    }

    fn on_args_fragment(&self, fragment: &str, now: Instant) {
        let preview = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.args.push_str(fragment);
            if !due(st.last_preview_emit, now) {
                return;
            }
            let Some(partial) = parse_partial(&st.args) else { return };
            let preview = preview_from_partial(&partial);
            if st.last_preview.as_ref() == Some(&preview) {
                return;
            }
            st.last_preview_emit = Some(now);
            st.last_preview = Some(preview.clone());
            preview
        };
        (self.sink)(BlockStreamUpdate::Preview(preview));
    }

    fn on_reasoning_fragment(&self, fragment: &str, now: Instant) {
        let chars = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.reasoning_chars += fragment.chars().count();
            if !due(st.last_reasoning_emit, now) {
                return;
            }
            st.last_reasoning_emit = Some(now);
            st.reasoning_chars
        };
        (self.sink)(BlockStreamUpdate::Reasoning { chars });
    }
}

impl AgentHook for BlockStreamHook {
    async fn on_tool_call_delta(&self, _ctx: &HookContext, event: ToolCallDelta<'_>) -> ObservationAction {
        self.on_args_fragment(event.delta, Instant::now());
        ObservationAction::Continue
    }

    async fn on_reasoning_delta(&self, _ctx: &HookContext, event: ReasoningDelta<'_>) -> ObservationAction {
        self.on_reasoning_fragment(event.delta, Instant::now());
        ObservationAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collecting_sink() -> (BlockStreamSink, Arc<Mutex<Vec<BlockStreamUpdate>>>) {
        let seen: Arc<Mutex<Vec<BlockStreamUpdate>>> = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        (Arc::new(move |u| seen2.lock().unwrap().push(u)), seen)
    }

    #[test]
    fn micro_theory_streams_its_whitelisted_prose() {
        let partial = json!({
            "blockType": "anchored_micro_theory", "title": "Recursión", "intuitiveHook": "Imagina", "systemRule": "r",
            "frequentError": "e", "somethingElse": "no", "visualAid": {"x": 1}
        });
        let preview = preview_from_partial(&partial);
        assert_eq!(preview["blockType"], "anchored_micro_theory");
        assert_eq!(preview["fields"], json!({"title": "Recursión", "intuitiveHook": "Imagina", "systemRule": "r", "frequentError": "e"}));
        assert!(preview["fields"].get("somethingElse").is_none());
        assert!(preview["fields"].get("visualAid").is_none());
    }

    /// THE safety property: answer keys and gate prose never reach the client
    /// through the stream, whatever order the model writes its fields in.
    #[test]
    fn gates_and_templates_never_leak_content_or_answer_keys() {
        let gate = json!({
            "blockType": "interactive_prediction_gate", "question": "¿Cuál?", "options": ["A", "B"], "correctOption": "B",
            "conceptualFeedbackMap": {"A": "mal"}
        });
        let p = preview_from_partial(&gate);
        assert_eq!(p["blockType"], "interactive_prediction_gate");
        assert_eq!(p["fields"], json!({}), "a gate streams no content at all");
        let flat = p.to_string();
        assert!(!flat.contains("correctOption") && !flat.contains("¿Cuál?") && !flat.contains("\"B\""));

        // A template (not a DynamicBlockType): its raw args hold `isTrue`.
        let tpl = json!({"blockType": "tpl_true_false", "statement": "s", "isTrue": true, "explanation": "porque sí"});
        let p = preview_from_partial(&tpl);
        assert_eq!(p["blockType"], Value::Null);
        assert_eq!(p["fields"], json!({}));
        assert!(!p.to_string().contains("porque"));

        // Answer-key fields are not forwarded even on a whitelisted type.
        let sneaky = json!({"blockType": "anchored_micro_theory", "title": "t", "correctOption": "B", "modelSolution": "x"});
        let p = preview_from_partial(&sneaky);
        assert!(!p.to_string().contains("correctOption") && !p.to_string().contains("modelSolution"));
    }

    #[test]
    fn a_half_written_block_type_is_not_forwarded_and_fields_wait_for_it() {
        let p = preview_from_partial(&json!({"blockType": "anchored_mi", "title": "pronto"}));
        assert_eq!(p["blockType"], Value::Null);
        assert_eq!(p["fields"], json!({}), "no content until the type is known and whitelisted");
        let p = preview_from_partial(&json!({"title": "antes del tipo"}));
        assert_eq!(p["fields"], json!({}));
    }

    #[test]
    fn long_fields_are_cut() {
        let long = "x".repeat(MAX_FIELD_CHARS + 500);
        let p = preview_from_partial(&json!({"blockType": "anchored_micro_theory", "title": long}));
        assert_eq!(p["fields"]["title"].as_str().unwrap().chars().count(), MAX_FIELD_CHARS);
    }

    #[test]
    fn non_string_values_in_whitelisted_fields_are_ignored() {
        let p = preview_from_partial(&json!({"blockType": "anchored_micro_theory", "title": 12, "systemRule": ["a"]}));
        assert_eq!(p["fields"], json!({}));
    }

    #[test]
    fn throttle_lets_the_first_through_then_waits_the_interval() {
        let t0 = Instant::now();
        assert!(due(None, t0));
        assert!(!due(Some(t0), t0 + Duration::from_millis(149)));
        assert!(due(Some(t0), t0 + MIN_EMIT_INTERVAL));
    }

    #[test]
    fn fragments_accumulate_and_the_sink_sees_a_growing_sanitized_preview() {
        let (sink, seen) = collecting_sink();
        let hook = BlockStreamHook::new(sink);
        let t0 = Instant::now();
        hook.on_args_fragment(r#"{"blockType":"anchored_micro_theory","ti"#, t0);
        hook.on_args_fragment(r#"tle":"Recurs"#, t0 + Duration::from_millis(10)); // throttled: buffered only
        hook.on_args_fragment(r#"ión","correctOption":"B"}"#, t0 + Duration::from_millis(400));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "first fragment + the one past the interval; the middle one was throttled");
        let BlockStreamUpdate::Preview(first) = &seen[0] else { panic!("preview expected") };
        assert_eq!(first["blockType"], "anchored_micro_theory");
        assert_eq!(first["fields"], json!({}), "title not written yet");
        let BlockStreamUpdate::Preview(last) = &seen[1] else { panic!("preview expected") };
        assert_eq!(last["fields"]["title"], "Recursión", "throttled fragments are not lost, only coalesced");
        assert!(!last.to_string().contains("correctOption"));
    }

    #[test]
    fn identical_previews_are_not_resent() {
        let (sink, seen) = collecting_sink();
        let hook = BlockStreamHook::new(sink);
        let t0 = Instant::now();
        hook.on_args_fragment(r#"{"blockType":"anchored_micro_theory","title":"A"#, t0);
        // Same visible preview (an unwhitelisted field changed only).
        hook.on_args_fragment(r#"","visualAid":{"x":1"#, t0 + Duration::from_secs(1));
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn reasoning_reports_only_an_amount_and_is_throttled() {
        let (sink, seen) = collecting_sink();
        let hook = BlockStreamHook::new(sink);
        let t0 = Instant::now();
        hook.on_reasoning_fragment("abc", t0);
        hook.on_reasoning_fragment("de", t0 + Duration::from_millis(50));
        hook.on_reasoning_fragment("fgh", t0 + Duration::from_millis(300));
        let seen = seen.lock().unwrap();
        assert_eq!(*seen, vec![BlockStreamUpdate::Reasoning { chars: 3 }, BlockStreamUpdate::Reasoning { chars: 8 }]);
    }
}
