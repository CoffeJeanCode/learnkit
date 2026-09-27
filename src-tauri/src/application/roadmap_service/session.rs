//! Session-state helpers shared across the flow: the sealed-roadmap
//! assembly, the persisted chat log and the timestamp helper.
use super::*;
use super::synth::*;

pub(super) fn build_sealed_roadmap(
    session: &RoadmapSession,
    profile: LearnerProfileCard,
    syllabus: RoadmapSyllabusPackage,
    diagnostic_battery: Option<DiagnosticBattery>,
) -> SealedRoadmap {
    let diagnostic_summary = session.diagnostic_summary_card.clone().unwrap_or_else(synth_diagnostic_card);
    SealedRoadmap {
        schema_version: "1.0".to_string(),
        package_id: Uuid::new_v4().to_string(),
        session_id: session.session_id.clone(),
        generated_at_ms: now_ms(),
        learner_profile: profile,
        diagnostic_summary,
        syllabus,
        diagnostic_battery,
    }
}

/// Appends the raw message to `session.draft.messages` — the only memory
/// mechanism left across stateless turns now that there's no incremental
/// `state_patch` (the agent either has everything and acts, or asks one
/// consolidated question). Re-sent as context on every turn so the model
/// doesn't lose what was already said, and used verbatim (last message) as
/// the force-close fallback's best-effort topic guess.
pub(super) fn record_message(session: &mut RoadmapSession, user_message: &str) {
    if !session.draft.is_object() {
        session.draft = serde_json::json!({"messages": []});
    }
    let obj = session.draft.as_object_mut().expect("just ensured object");
    let entry = obj.entry("messages").or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let serde_json::Value::Array(arr) = entry {
        arr.push(serde_json::Value::String(user_message.to_string()));
    }
}

/// Defensive ceiling on the conversation log: the 3-gate flow is bounded
/// (~a handful of turns per session), so this only guards against a
/// pathological loop keeping unbounded JSON on disk. Keeps the most recent
/// entries — old context matters less than new.
pub(super) const MAX_LOG_ENTRIES: usize = 100;

pub(super) fn trim_message_log(session: &mut RoadmapSession) {
    if session.messages.len() > MAX_LOG_ENTRIES {
        let overflow = session.messages.len() - MAX_LOG_ENTRIES;
        session.messages.drain(..overflow);
    }
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

