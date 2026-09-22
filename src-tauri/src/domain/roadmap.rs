use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The 4 sequential phases of the Roadmap & Syllabus Diagnostic Agent.
/// Order matters: `next()` walks this list; there is no phase after `Negotiation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadmapPhase {
    Exploration,
    Diagnostic,
    Syllabus,
    Negotiation,
}

impl RoadmapPhase {
    /// Next phase in sequence, or `None` if already at the last phase.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Exploration => Some(Self::Diagnostic),
            Self::Diagnostic => Some(Self::Syllabus),
            Self::Syllabus => Some(Self::Negotiation),
            Self::Negotiation => None,
        }
    }
}

/// What the agent believes should happen next. The gate decision (whether the
/// phase actually advances) is made in [`crate::application::RoadmapService`],
/// never inferred from this alone — see `dod` on [`RoadmapTurnEnvelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    AskQuestion,
    RequestClarification,
    PhaseComplete,
    AwaitConfirmation,
    FinalDelivery,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnFlags {
    #[serde(default)]
    pub clarification_attempts: u8,
    #[serde(default)]
    pub assumed_fields: Vec<String>,
    #[serde(default)]
    pub revision_requested: Option<RoadmapPhase>,
}

/// The machine-readable block the agent must emit at the end of every turn,
/// fenced as ```roadmap_state ... ``` in its text response. See
/// `parse_turn_envelope` for extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapTurnEnvelope {
    pub schema_version: String,
    pub session_id: String,
    pub phase: RoadmapPhase,
    #[serde(default)]
    pub dod: HashMap<String, bool>,
    #[serde(default = "default_patch")]
    pub state_patch: serde_json::Value,
    pub next_action: NextAction,
    #[serde(default)]
    pub flags: TurnFlags,
}

fn default_patch() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

/// Extracts and parses the fenced `roadmap_state` block from an agent's raw
/// text output. Returns `InvalidInput` if the block is missing or not valid
/// JSON — callers should treat this as a correctable turn, not a hard failure
/// (see `RoadmapService::handle_turn`).
pub fn parse_turn_envelope(text: &str) -> Result<RoadmapTurnEnvelope, String> {
    const FENCE_OPEN: &str = "```roadmap_state";
    const FENCE_CLOSE: &str = "```";

    let start = text.find(FENCE_OPEN).ok_or("missing ```roadmap_state block")?;
    let after_open = &text[start + FENCE_OPEN.len()..];
    let end = after_open.find(FENCE_CLOSE).ok_or("unterminated ```roadmap_state block")?;
    let json_str = after_open[..end].trim();

    serde_json::from_str::<RoadmapTurnEnvelope>(json_str).map_err(|e| format!("invalid roadmap_state JSON: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillLevel {
    Novice,
    Intermediate,
    Advanced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerProfileState {
    pub declared_topic: String,
    pub time_horizon_weeks: u16,
    pub hours_per_week: f32,
    pub self_perceived_level: SkillLevel,
    pub level_justification: String,
    #[serde(default)]
    pub prior_experience: Vec<String>,
    #[serde(default)]
    pub applied_interests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub concept: String,
    pub evidence: String,
    pub is_prerequisite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticAssessmentSummary {
    pub critical_gaps: Vec<KnowledgeGap>,
    #[serde(default)]
    pub confirmed_strengths: Vec<String>,
    pub capstone_options_presented: Vec<String>,
    pub capstone_selected: String,
    pub difficulty_calibration_note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BloomLevel {
    Apply,
    Analyze,
    Evaluate,
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    PredictionBeforeReveal,
    MicroSimulator,
    FailureAudit,
    MetacognitiveReflection,
    ConceptCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub bloom_level: BloomLevel,
    pub learning_objective: String,
    pub authentic_challenge: String,
    pub notebook_block_plan: Vec<BlockType>,
    pub estimated_hours: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftSyllabusTree {
    pub milestones: Vec<Milestone>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageStatus {
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approved_verbatim_quote: String,
    pub approved_at_turn: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionEntry {
    pub at_turn: u32,
    pub reverted_to_phase: RoadmapPhase,
    pub reason: String,
    #[serde(default)]
    pub fields_invalidated: Vec<String>,
}

/// The Phase 4 deliverable: the full contract handed to the Notebook Builder
/// agent. Must be self-sufficient — no empty fields, no need to re-ask the
/// learner anything. Sealed once via [`crate::application::RoadmapService`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapPackageFinal {
    pub schema_version: String,
    pub package_id: String,
    pub session_id: String,
    pub generated_at_ms: u64,
    pub status: PackageStatus,

    pub learner_profile: LearnerProfileState,
    pub diagnostic_summary: DiagnosticAssessmentSummary,
    pub syllabus: DraftSyllabusTree,

    pub executive_summary: String,
    pub approval: ApprovalRecord,
    #[serde(default)]
    pub revision_log: Vec<RevisionEntry>,
    pub handoff_notes_for_notebook_builder: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Sealed,
}

/// Persisted, cross-turn state for one roadmap diagnostic conversation.
/// The backend orchestrator itself is stateless per call (see
/// [`crate::orchestration::Orchestrator::run_agent`]); this struct is what
/// carries continuity between turns, loaded/saved by
/// [`crate::application::RoadmapService`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapSession {
    pub session_id: String,
    pub phase: RoadmapPhase,
    pub status: SessionStatus,
    pub turn_count_in_phase: u32,
    #[serde(default)]
    pub dod: HashMap<String, bool>,
    #[serde(default)]
    pub clarification_attempts: HashMap<String, u8>,
    /// Accumulated fields for each phase deliverable, merged shallowly from
    /// `state_patch` on every turn. Deserialized into the typed struct only
    /// when needed (e.g. sealing `RoadmapPackageFinal`) — untyped accumulation
    /// avoids hand-writing a field-by-field merge for every deliverable shape.
    #[serde(default = "empty_object")]
    pub learner_profile: serde_json::Value,
    #[serde(default = "empty_object")]
    pub diagnostic_summary: serde_json::Value,
    #[serde(default = "empty_object")]
    pub syllabus: serde_json::Value,
    #[serde(default = "empty_object")]
    pub negotiation: serde_json::Value,
    pub final_package: Option<RoadmapPackageFinal>,
    #[serde(default)]
    pub revision_log: Vec<RevisionEntry>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

impl RoadmapSession {
    pub fn new(session_id: String, now_ms: u64) -> Self {
        Self {
            session_id,
            phase: RoadmapPhase::Exploration,
            status: SessionStatus::Active,
            turn_count_in_phase: 0,
            dod: HashMap::new(),
            clarification_attempts: HashMap::new(),
            learner_profile: empty_object(),
            diagnostic_summary: empty_object(),
            syllabus: empty_object(),
            negotiation: empty_object(),
            final_package: None,
            revision_log: Vec::new(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        }
    }

    /// Shallow-merges `patch`'s top-level keys into the deliverable bucket for
    /// `phase`. Non-object patches are ignored (defensive: a malformed patch
    /// must never corrupt already-confirmed state).
    pub fn merge_patch(&mut self, phase: RoadmapPhase, patch: serde_json::Value) {
        let serde_json::Value::Object(patch_map) = patch else { return };
        let bucket = match phase {
            RoadmapPhase::Exploration => &mut self.learner_profile,
            RoadmapPhase::Diagnostic => &mut self.diagnostic_summary,
            RoadmapPhase::Syllabus => &mut self.syllabus,
            RoadmapPhase::Negotiation => &mut self.negotiation,
        };
        if !bucket.is_object() {
            *bucket = empty_object();
        }
        let obj = bucket.as_object_mut().expect("just ensured object");
        for (k, v) in patch_map {
            obj.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_envelope() {
        let text = r#"Claro, cuéntame más sobre tu experiencia previa.

```roadmap_state
{
  "schema_version": "1.0",
  "session_id": "abc123",
  "phase": "exploration",
  "dod": {"objetivo_terminal_confirmado": false},
  "state_patch": {"declared_topic": "Rust"},
  "next_action": "ask_question",
  "flags": {"clarification_attempts": 0, "assumed_fields": []}
}
```"#;
        let env = parse_turn_envelope(text).expect("parses");
        assert_eq!(env.session_id, "abc123");
        assert_eq!(env.phase, RoadmapPhase::Exploration);
        assert_eq!(env.next_action, NextAction::AskQuestion);
        assert_eq!(env.dod.get("objetivo_terminal_confirmado"), Some(&false));
    }

    #[test]
    fn missing_block_is_rejected() {
        let err = parse_turn_envelope("just a plain reply, no block").unwrap_err();
        assert!(err.contains("missing"));
    }

    #[test]
    fn malformed_json_is_rejected() {
        let text = "```roadmap_state\n{not valid json\n```";
        let err = parse_turn_envelope(text).unwrap_err();
        assert!(err.contains("invalid roadmap_state JSON"));
    }

    #[test]
    fn merge_patch_upserts_fields_without_clobbering_other_phases() {
        let mut session = RoadmapSession::new("s1".to_string(), 0);
        session.merge_patch(
            RoadmapPhase::Exploration,
            serde_json::json!({"declared_topic": "Rust", "hours_per_week": 5}),
        );
        session.merge_patch(RoadmapPhase::Exploration, serde_json::json!({"hours_per_week": 8}));

        assert_eq!(session.learner_profile["declared_topic"], "Rust");
        assert_eq!(session.learner_profile["hours_per_week"], 8);
        assert_eq!(session.diagnostic_summary, empty_object());
    }

    #[test]
    fn phase_sequence_ends_at_negotiation() {
        assert_eq!(RoadmapPhase::Exploration.next(), Some(RoadmapPhase::Diagnostic));
        assert_eq!(RoadmapPhase::Diagnostic.next(), Some(RoadmapPhase::Syllabus));
        assert_eq!(RoadmapPhase::Syllabus.next(), Some(RoadmapPhase::Negotiation));
        assert_eq!(RoadmapPhase::Negotiation.next(), None);
    }
}
