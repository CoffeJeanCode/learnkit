//! Append-only history of what a student actually DEMONSTRATED, per skill.
//!
//! Progress shown to the learner (and any reward built on it) must point at
//! evidence, not at "the block was shown" or "a counter went up". Every graded
//! attempt, self-reported retrieval and graded reflection appends ONE row
//! here; nothing is ever updated or deleted, so any later state ("solved",
//! "retained", "applied") can be re-derived — and re-defined — from the rows.
//!
//! Skill identity: a skill is a class (micromodule). Its id is the class id —
//! already stable and unique, already the key of the spaced-retrieval queue,
//! and it carries the capability statement (`ClassRecord::objective`). A
//! finer-grained taxonomy (several skills per class) can be added later
//! without touching old rows: `block_id` already pins each row to the exact
//! activity that produced it.

use serde::{Deserialize, Serialize};

use crate::domain::notebook::{DynamicBlockType, GateCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A conceptual gate (prediction, branching scenario).
    ConceptualGate,
    /// A practice gate (error audit, hands-on mission).
    PracticeGate,
    /// A spaced-retrieval prompt answered after the concept was learned.
    Retrieval,
    /// The closing reflection.
    Closure,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConceptualGate => "conceptual_gate",
            Self::PracticeGate => "practice_gate",
            Self::Retrieval => "retrieval",
            Self::Closure => "closure",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "conceptual_gate" => Some(Self::ConceptualGate),
            "practice_gate" => Some(Self::PracticeGate),
            "retrieval" => Some(Self::Retrieval),
            "closure" => Some(Self::Closure),
            _ => None,
        }
    }

    /// The kind of evidence a block type produces when graded.
    pub fn for_block(block_type: DynamicBlockType) -> Option<Self> {
        match block_type.gate_category() {
            Some(GateCategory::Conceptual) => Some(Self::ConceptualGate),
            Some(GateCategory::Practice) => Some(Self::PracticeGate),
            None => match block_type {
                DynamicBlockType::MetacognitiveClosure => Some(Self::Closure),
                DynamicBlockType::SpacedInterleavedRetrieval => Some(Self::Retrieval),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Passed,
    Failed,
    /// The student could not pass and the notebook moved on with a different
    /// approach — "could continue", NOT "demonstrated".
    Escalated,
    SelfRecalled,
    SelfForgot,
}

impl EvidenceOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Escalated => "escalated",
            Self::SelfRecalled => "self_recalled",
            Self::SelfForgot => "self_forgot",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "escalated" => Some(Self::Escalated),
            "self_recalled" => Some(Self::SelfRecalled),
            "self_forgot" => Some(Self::SelfForgot),
            _ => None,
        }
    }

    /// Whether this outcome counts as the student having shown the skill.
    /// Self-reports are deliberately NOT demonstrations on their own.
    pub fn is_demonstration(self) -> bool {
        self == Self::Passed
    }
}

/// One criterion of a rubric as judged by the grader: what was asked, whether
/// the submission met it, and the evidence found (or what is still missing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricCriterion {
    pub criterion: String,
    pub met: bool,
    #[serde(default)]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEvidence {
    pub id: String,
    /// The class id (see module doc).
    pub skill_id: String,
    pub course_id: String,
    pub block_id: Option<String>,
    pub kind: EvidenceKind,
    pub outcome: EvidenceOutcome,
    /// 1-based attempt on this activity (1 for retrieval/closure first tries).
    pub attempt_number: u32,
    /// Feedback rounds (hints / consequence explanations) the student had
    /// already received on this activity BEFORE this attempt.
    pub hints_shown: u32,
    /// `domain::scaffolding::SupportLevel` the activity was generated under
    /// (`full|guided|faded|independent`); `None` when it can't be derived.
    pub support_level: Option<String>,
    /// Criteria the grader judged; empty for deterministic gates and
    /// self-reports.
    #[serde(default)]
    pub rubric: Vec<RubricCriterion>,
    /// The activity was a transfer challenge (a NEW case, different
    /// constraints, no hints) — the only evidence "applied" is built from.
    #[serde(default)]
    pub is_transfer: bool,
    /// Study version (`domain::study`) in force when the row was written —
    /// stamped by the store, so a mid-study switch stays visible in the data.
    #[serde(default)]
    pub variant: Option<String>,
    pub created_at_ms: i64,
}

impl SkillEvidence {
    /// A fresh row stamped with a new id and the current time; chain the
    /// `with_*` setters for the optional parts.
    pub fn new(skill_id: &str, course_id: &str, kind: EvidenceKind, outcome: EvidenceOutcome) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            skill_id: skill_id.to_string(),
            course_id: course_id.to_string(),
            block_id: None,
            kind,
            outcome,
            attempt_number: 1,
            hints_shown: 0,
            support_level: None,
            rubric: Vec::new(),
            is_transfer: false,
            variant: None,
            created_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }
    }

    pub fn with_block(mut self, block_id: &str) -> Self {
        self.block_id = Some(block_id.to_string());
        self
    }

    pub fn with_attempt(mut self, attempt_number: u32, hints_shown: u32) -> Self {
        self.attempt_number = attempt_number;
        self.hints_shown = hints_shown;
        self
    }

    pub fn with_support_level(mut self, level: Option<&str>) -> Self {
        self.support_level = level.map(str::to_string);
        self
    }

    pub fn with_transfer(mut self, is_transfer: bool) -> Self {
        self.is_transfer = is_transfer;
        self
    }

    pub fn with_rubric(mut self, rubric: Vec<RubricCriterion>) -> Self {
        self.rubric = rubric;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_outcomes_round_trip_through_their_storage_strings() {
        for k in [EvidenceKind::ConceptualGate, EvidenceKind::PracticeGate, EvidenceKind::Retrieval, EvidenceKind::Closure] {
            assert_eq!(EvidenceKind::parse(k.as_str()), Some(k));
        }
        for o in [EvidenceOutcome::Passed, EvidenceOutcome::Failed, EvidenceOutcome::Escalated, EvidenceOutcome::SelfRecalled, EvidenceOutcome::SelfForgot] {
            assert_eq!(EvidenceOutcome::parse(o.as_str()), Some(o));
        }
        assert_eq!(EvidenceKind::parse("nope"), None);
    }

    #[test]
    fn each_gradable_block_type_maps_to_one_kind_and_content_blocks_to_none() {
        use DynamicBlockType::*;
        assert_eq!(EvidenceKind::for_block(InteractivePredictionGate), Some(EvidenceKind::ConceptualGate));
        assert_eq!(EvidenceKind::for_block(BranchingScenarioChallenge), Some(EvidenceKind::ConceptualGate));
        assert_eq!(EvidenceKind::for_block(HeuristicErrorAudit), Some(EvidenceKind::PracticeGate));
        assert_eq!(EvidenceKind::for_block(HandsOnMission), Some(EvidenceKind::PracticeGate));
        assert_eq!(EvidenceKind::for_block(MetacognitiveClosure), Some(EvidenceKind::Closure));
        assert_eq!(EvidenceKind::for_block(AnchoredMicroTheory), None);
    }

    #[test]
    fn only_a_pass_is_a_demonstration() {
        assert!(EvidenceOutcome::Passed.is_demonstration());
        for o in [EvidenceOutcome::Failed, EvidenceOutcome::Escalated, EvidenceOutcome::SelfRecalled, EvidenceOutcome::SelfForgot] {
            assert!(!o.is_demonstration(), "{o:?} must not count as demonstrated");
        }
    }
}
