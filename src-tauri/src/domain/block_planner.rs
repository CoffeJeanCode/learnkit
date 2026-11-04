//! Deterministic plan for the NEXT block of a class: which block type (and
//! mode) belongs at this position, and how the block should be PRESENTED.
//!
//! The decision used to live entirely in the generator's prompt ("choose the
//! next block given masteryStatus, scaffolding, compositionHint…"), which cost
//! prompt tokens on every call and made personalization depend on the model
//! re-deriving the same rules each time. Everything here is a pure function of
//! state the system already computes, so:
//!
//! - when the position is DETERMINED (retrieval first, transfer mission,
//!   closure, a check before a third stacked theory, the profile chain…) the
//!   plan names the type and the generator gets a short, type-specific prompt
//!   (`agents::block_generator_agent::system_prompt_for`) and only writes
//!   content;
//! - when it is not (`block_type: None`), the model decides exactly as before
//!   with the full prompt;
//! - the PRESENTATION (density / emphasis / layout) is derived from the same
//!   signals and attached to the persisted block, so the UI adapts without the
//!   model spending output tokens on layout.
//!
//! The plan is advisory-firm: grounding still validates whatever comes back,
//! and a retry falls back to the full prompt, so a wrong plan can cost a retry
//! but never a dead end.

use serde::{Deserialize, Serialize};

use crate::domain::lesson_composition::DisciplineProfile;
use crate::domain::notebook::{BlockStatus, DynamicBlockType, InteractionMode, MissionSupportLevel, NotebookBlock};
use crate::domain::scaffolding::{self, Momentum, ScaffoldingPlan, SupportLevel};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Tight spacing: a student who is cruising.
    Compact,
    #[default]
    Comfortable,
    /// Generous spacing and larger type: a student who is struggling.
    Spacious,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emphasis {
    /// The figure leads (intuition-weak / low-friction students).
    Visual,
    #[default]
    Balanced,
    /// The task leads; reading material is compact (a student ahead of the pace).
    Practice,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Stacked,
    /// Figure beside the text on wide screens (only meaningful when the block has a visual).
    Split,
}

/// How the UI should present one block. Stored in the block's content as
/// `presentation`; the frontend only ever styles with it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub density: Density,
    pub emphasis: Emphasis,
    pub layout: Layout,
}

/// Learner signals that shape the plan (all already computed elsewhere).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LearnerSignals {
    pub heavy_scaffolding: bool,
    pub friction_low: bool,
    /// The diagnostic flagged weak intuition (`weakPoints` has `intuition`).
    pub intuition_weak: bool,
}

pub struct PlanInputs<'a> {
    pub existing: &'a [NotebookBlock],
    pub has_passed_conceptual: bool,
    pub has_passed_practice: bool,
    pub has_due_retrieval: bool,
    pub is_escalation: bool,
    pub force_close: bool,
    /// `lesson_composition::suggest_next_block` for this position, if a profile was inferred.
    pub suggested: Option<DynamicBlockType>,
    pub profile: Option<DisciplineProfile>,
    pub scaffolding: &'a ScaffoldingPlan,
    pub learner: LearnerSignals,
    pub dominant: &'a [DynamicBlockType],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockPlan {
    /// `Some` = the position is determined (lean prompt); `None` = the model decides.
    pub block_type: Option<DynamicBlockType>,
    /// Why — a stable tag for traces and tests.
    pub reason: &'static str,
    pub interaction_mode: Option<InteractionMode>,
    /// For `hands_on_mission`: the support level it must declare.
    pub mission_support: Option<MissionSupportLevel>,
    pub is_transfer: bool,
    pub presentation: Presentation,
}

impl BlockPlan {
    fn new(block_type: Option<DynamicBlockType>, reason: &'static str, presentation: Presentation) -> Self {
        Self { block_type, reason, interaction_mode: None, mission_support: None, is_transfer: false, presentation }
    }

    /// A plan for a block whose type is fixed by someone else (a repair that
    /// replaces a broken block in place).
    pub fn fixed(block_type: DynamicBlockType, presentation: Presentation) -> Self {
        Self::new(Some(block_type), "fixed_type", presentation)
    }

    /// The `blockPlan` object appended to the generator's input when the type
    /// is determined (`None` when the model decides).
    pub fn to_hint_json(&self) -> Option<serde_json::Value> {
        let t = self.block_type?;
        let mut v = serde_json::json!({
            "requiredBlockType": t.as_str(),
            "reason": self.reason,
            "guidance": "El sistema ya decidió el tipo de este bloque: escribe SOLO su contenido. No cambies de tipo salvo que sea imposible.",
        });
        if let Some(mode) = self.interaction_mode {
            v["suggestedInteractionMode"] = serde_json::json!(mode_str(mode));
        }
        if let Some(level) = self.mission_support {
            v["requiredSupportLevel"] = serde_json::json!(mission_level_str(level));
        }
        if self.is_transfer {
            v["isTransfer"] = serde_json::json!(true);
        }
        Some(v)
    }
}

fn mode_str(m: InteractionMode) -> &'static str {
    match m {
        InteractionMode::MultipleChoice => "multiple_choice",
        InteractionMode::CausalChainOrder => "causal_chain_order",
        InteractionMode::BoundaryCaseAudit => "boundary_case_audit",
    }
}

fn mission_level_str(l: MissionSupportLevel) -> &'static str {
    match l {
        MissionSupportLevel::Full => "full",
        MissionSupportLevel::Guided => "guided",
        MissionSupportLevel::Faded => "faded",
        MissionSupportLevel::Independent => "independent",
    }
}

fn mission_level(l: SupportLevel) -> MissionSupportLevel {
    match l {
        SupportLevel::Full => MissionSupportLevel::Full,
        SupportLevel::Guided => MissionSupportLevel::Guided,
        SupportLevel::Faded => MissionSupportLevel::Faded,
        SupportLevel::Independent => MissionSupportLevel::Independent,
    }
}

fn is_teaching(t: DynamicBlockType) -> bool {
    matches!(t, DynamicBlockType::AnchoredMicroTheory | DynamicBlockType::DeclarativeVisualDiagram)
}

/// Presentation for the block about to be generated, from the learner's state.
pub fn presentation_for(learner: LearnerSignals, momentum: Momentum) -> Presentation {
    let density = if learner.heavy_scaffolding || learner.friction_low || momentum == Momentum::Struggling {
        Density::Spacious
    } else if momentum == Momentum::Rising {
        Density::Compact
    } else {
        Density::Comfortable
    };
    let emphasis = if learner.intuition_weak || learner.friction_low || learner.heavy_scaffolding {
        Emphasis::Visual
    } else if momentum == Momentum::Rising {
        Emphasis::Practice
    } else {
        Emphasis::Balanced
    };
    let layout = if emphasis == Emphasis::Visual { Layout::Split } else { Layout::Stacked };
    Presentation { density, emphasis, layout }
}

/// Interaction mode of a prediction gate: the profile's natural fit, rotated
/// away from the mode of the class's previous prediction gate so consecutive
/// gates don't feel like the same template.
fn pick_mode(profile: Option<DisciplineProfile>, existing: &[NotebookBlock]) -> InteractionMode {
    let preferred = match profile {
        Some(DisciplineProfile::SpatialBiological | DisciplineProfile::CyclicSystemic) => InteractionMode::CausalChainOrder,
        Some(DisciplineProfile::QuantitativeMath | DisciplineProfile::SoftwareDebugging) => InteractionMode::BoundaryCaseAudit,
        _ => InteractionMode::MultipleChoice,
    };
    let last_mode = existing
        .iter()
        .rev()
        .find(|b| b.block_type == DynamicBlockType::InteractivePredictionGate)
        .map(|b| match b.content_json.get("interactionMode").and_then(|v| v.as_str()) {
            Some("causal_chain_order") => InteractionMode::CausalChainOrder,
            Some("boundary_case_audit") => InteractionMode::BoundaryCaseAudit,
            _ => InteractionMode::MultipleChoice,
        });
    if last_mode != Some(preferred) {
        return preferred;
    }
    [InteractionMode::MultipleChoice, InteractionMode::BoundaryCaseAudit, InteractionMode::CausalChainOrder]
        .into_iter()
        .find(|m| *m != preferred)
        .unwrap_or(preferred)
}

/// The plan for the next block. See the module docs for the contract.
pub fn plan_next(i: &PlanInputs<'_>) -> BlockPlan {
    let presentation = presentation_for(i.learner, i.scaffolding.momentum);
    let firm = |t: DynamicBlockType, reason: &'static str| {
        let mut p = BlockPlan::new(Some(t), reason, presentation);
        match t {
            DynamicBlockType::InteractivePredictionGate => p.interaction_mode = Some(pick_mode(i.profile, i.existing)),
            DynamicBlockType::HandsOnMission => p.mission_support = Some(mission_level(i.scaffolding.support_level)),
            _ => {}
        }
        p
    };

    if i.is_escalation {
        return BlockPlan::new(None, "escalation", presentation);
    }
    if i.force_close {
        return firm(DynamicBlockType::MetacognitiveClosure, "forced_close");
    }
    let types: Vec<DynamicBlockType> = i.existing.iter().map(|b| b.block_type).collect();
    let last = types.last().copied();
    if types.is_empty() && i.has_due_retrieval {
        return firm(DynamicBlockType::SpacedInterleavedRetrieval, "due_retrieval");
    }
    if i.scaffolding.transfer_due {
        let mut p = firm(DynamicBlockType::HandsOnMission, "transfer_due");
        p.is_transfer = true;
        p.mission_support = Some(MissionSupportLevel::Independent);
        return p;
    }
    let mastery_complete = i.has_passed_conceptual && i.has_passed_practice;
    if mastery_complete && scaffolding::transfer_attempted(i.existing) {
        return firm(DynamicBlockType::MetacognitiveClosure, "mastery_complete");
    }
    let pick_practice_gate = |avoid: &[DynamicBlockType], last: Option<DynamicBlockType>| -> DynamicBlockType {
        let candidates = [
            DynamicBlockType::HandsOnMission,
            DynamicBlockType::HeuristicErrorAudit,
            DynamicBlockType::BranchingScenarioChallenge,
        ];
        for &c in &candidates {
            if !avoid.contains(&c) && Some(c) != last {
                return c;
            }
        }
        candidates.into_iter().find(|&c| Some(c) != last).unwrap_or(DynamicBlockType::HandsOnMission)
    };

    // Never stack a third unchecked teaching block: check what was taught.
    let teaching_run = types.iter().rev().take_while(|t| is_teaching(**t)).count();
    if teaching_run >= 2 {
        let gate = if i.has_passed_conceptual {
            pick_practice_gate(i.dominant, last)
        } else {
            DynamicBlockType::InteractivePredictionGate
        };
        let mut p = firm(gate, "check_before_stacking");
        if gate == DynamicBlockType::HandsOnMission {
            p.mission_support = Some(MissionSupportLevel::Guided);
        }
        return p;
    }
    // The profile chain, corrected for the rules grounding would reject anyway.
    if let Some(suggested) = i.suggested {
        let non_retrieval = types.iter().filter(|t| **t != DynamicBlockType::SpacedInterleavedRetrieval).count();
        let t = if suggested == DynamicBlockType::HeuristicErrorAudit && last == Some(DynamicBlockType::AnchoredMicroTheory) && non_retrieval == 1 {
            // Golden rule: never audit an error right after the first theory.
            DynamicBlockType::InteractivePredictionGate
        } else if i.dominant.contains(&suggested) {
            pick_practice_gate(i.dominant, last)
        } else {
            suggested
        };
        if !i.dominant.contains(&t) && Some(t) != last && !(t == DynamicBlockType::MetacognitiveClosure && !mastery_complete) {
            let reason = if t == suggested { "profile_chain" } else { "golden_rule" };
            return firm(t, reason);
        }
    }
    // Chain exhausted but a mastery dimension is still missing: go for it
    // unless that would repeat the previous block (then the model decides).
    if !mastery_complete {
        let missing = if !i.has_passed_conceptual {
            DynamicBlockType::InteractivePredictionGate
        } else {
            pick_practice_gate(i.dominant, last)
        };
        let gated_last = last.is_some_and(|t| t.is_gate()) && i.existing.last().is_some_and(|b| b.status != BlockStatus::Passed);
        if Some(missing) != last && !gated_last && types.iter().any(|t| is_teaching(*t)) {
            return firm(missing, "missing_mastery");
        }
    }
    BlockPlan::new(None, "model_decides", presentation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::NotebookBlock;

    fn block(t: DynamicBlockType, status: BlockStatus, content: serde_json::Value) -> NotebookBlock {
        NotebookBlock {
            id: format!("b-{}", t.as_str()),
            document_id: "d".into(),
            block_type: t,
            content_json: content,
            order_index: 0,
            status,
            concept_node_id: None,
            attempt_count: 0,
            last_feedback: None,
        }
    }

    fn plan_with(existing: &[NotebookBlock], f: impl FnOnce(&mut PlanInputs)) -> BlockPlan {
        let sp = scaffolding::plan(existing, false);
        let mut inputs = PlanInputs {
            existing,
            has_passed_conceptual: existing.iter().any(|b| b.status == BlockStatus::Passed && b.block_type.gate_category() == Some(crate::domain::notebook::GateCategory::Conceptual)),
            has_passed_practice: existing.iter().any(|b| b.status == BlockStatus::Passed && b.block_type.gate_category() == Some(crate::domain::notebook::GateCategory::Practice)),
            has_due_retrieval: false,
            is_escalation: false,
            force_close: false,
            suggested: None,
            profile: None,
            scaffolding: &sp,
            learner: LearnerSignals::default(),
            dominant: &[],
        };
        f(&mut inputs);
        plan_next(&inputs)
    }

    fn theory() -> NotebookBlock {
        block(DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready, serde_json::json!({}))
    }
    fn diagram() -> NotebookBlock {
        block(DynamicBlockType::DeclarativeVisualDiagram, BlockStatus::Ready, serde_json::json!({}))
    }
    fn gate(t: DynamicBlockType, status: BlockStatus) -> NotebookBlock {
        block(t, status, serde_json::json!({}))
    }

    #[test]
    fn escalation_and_unknown_positions_are_left_to_the_model() {
        let p = plan_with(&[theory()], |i| i.is_escalation = true);
        assert_eq!((p.block_type, p.reason), (None, "escalation"));
        // An empty class with nothing due: the model opens it.
        let p = plan_with(&[], |_| {});
        assert_eq!((p.block_type, p.reason), (None, "model_decides"));
        assert!(p.to_hint_json().is_none());
    }

    #[test]
    fn a_due_retrieval_opens_the_class_only_when_it_is_empty() {
        let p = plan_with(&[], |i| i.has_due_retrieval = true);
        assert_eq!(p.block_type, Some(DynamicBlockType::SpacedInterleavedRetrieval));
        let p = plan_with(&[theory()], |i| i.has_due_retrieval = true);
        assert_ne!(p.block_type, Some(DynamicBlockType::SpacedInterleavedRetrieval));
    }

    #[test]
    fn transfer_then_closure_follow_mastery() {
        let passed = [
            theory(),
            gate(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed),
            gate(DynamicBlockType::HeuristicErrorAudit, BlockStatus::Passed),
        ];
        let p = plan_with(&passed, |_| {});
        assert_eq!(p.block_type, Some(DynamicBlockType::HandsOnMission));
        assert!(p.is_transfer);
        assert_eq!(p.mission_support, Some(MissionSupportLevel::Independent));
        let hint = p.to_hint_json().unwrap();
        assert_eq!(hint["requiredBlockType"], "hands_on_mission");
        assert_eq!(hint["isTransfer"], true);

        let mut done = passed.to_vec();
        done.push(block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, serde_json::json!({"isTransfer": true})));
        let p = plan_with(&done, |_| {});
        assert_eq!((p.block_type, p.reason), (Some(DynamicBlockType::MetacognitiveClosure), "mastery_complete"));
    }

    #[test]
    fn a_third_teaching_block_is_replaced_by_a_check() {
        let p = plan_with(&[theory(), diagram()], |i| i.suggested = Some(DynamicBlockType::AnchoredMicroTheory));
        assert_eq!((p.block_type, p.reason), (Some(DynamicBlockType::InteractivePredictionGate), "check_before_stacking"));
        assert!(p.interaction_mode.is_some());
    }

    #[test]
    fn the_golden_rule_blocks_an_audit_right_after_the_first_theory() {
        let p = plan_with(&[theory()], |i| i.suggested = Some(DynamicBlockType::HeuristicErrorAudit));
        assert_eq!((p.block_type, p.reason), (Some(DynamicBlockType::InteractivePredictionGate), "golden_rule"));
        // …but the debugging chain may OPEN with the audit.
        let p = plan_with(&[], |i| i.suggested = Some(DynamicBlockType::HeuristicErrorAudit));
        assert_eq!((p.block_type, p.reason), (Some(DynamicBlockType::HeuristicErrorAudit), "profile_chain"));
    }

    #[test]
    fn the_suggestion_is_never_a_repeat_nor_an_early_closure() {
        // A suggested repeat is dropped; the plan then chases the missing gate instead.
        let p = plan_with(&[theory()], |i| i.suggested = Some(DynamicBlockType::AnchoredMicroTheory));
        assert_ne!(p.block_type, Some(DynamicBlockType::AnchoredMicroTheory));
        let p = plan_with(&[theory()], |i| i.suggested = Some(DynamicBlockType::MetacognitiveClosure));
        assert_ne!(p.block_type, Some(DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn a_missing_mastery_dimension_is_chased_after_teaching() {
        let p = plan_with(&[theory()], |_| {});
        assert_eq!((p.block_type, p.reason), (Some(DynamicBlockType::InteractivePredictionGate), "missing_mastery"));
        // No teaching yet → nothing to check → the model opens the class.
        assert_eq!(plan_with(&[], |_| {}).block_type, None);
        // Hands-on carries the scaffolding rung it must declare.
        let after_concept = [theory(), gate(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed)];
        let p = plan_with(&after_concept, |_| {});
        assert_eq!(p.block_type, Some(DynamicBlockType::HandsOnMission));
        assert_eq!(p.mission_support, Some(MissionSupportLevel::Faded));
        assert_eq!(p.to_hint_json().unwrap()["requiredSupportLevel"], "faded");
    }

    #[test]
    fn prediction_modes_follow_the_profile_and_rotate() {
        let p = plan_with(&[theory()], |i| {
            i.profile = Some(DisciplineProfile::SpatialBiological);
            i.suggested = Some(DynamicBlockType::InteractivePredictionGate);
        });
        assert_eq!(p.interaction_mode, Some(InteractionMode::CausalChainOrder));
        let prev = block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed, serde_json::json!({"interactionMode": "causal_chain_order"}));
        let p = plan_with(&[theory(), prev, diagram()], |i| {
            i.profile = Some(DisciplineProfile::SpatialBiological);
            i.suggested = Some(DynamicBlockType::InteractivePredictionGate);
        });
        assert_ne!(p.interaction_mode, Some(InteractionMode::CausalChainOrder), "same mode twice in a row is rotated away");
    }

    #[test]
    fn presentation_adapts_to_the_learner() {
        let calm = presentation_for(LearnerSignals::default(), Momentum::Steady);
        assert_eq!(calm, Presentation { density: Density::Comfortable, emphasis: Emphasis::Balanced, layout: Layout::Stacked });
        let cruising = presentation_for(LearnerSignals::default(), Momentum::Rising);
        assert_eq!((cruising.density, cruising.emphasis), (Density::Compact, Emphasis::Practice));
        let weak = presentation_for(LearnerSignals { intuition_weak: true, ..Default::default() }, Momentum::Steady);
        assert_eq!((weak.emphasis, weak.layout), (Emphasis::Visual, Layout::Split));
        let struggling = presentation_for(LearnerSignals::default(), Momentum::Struggling);
        assert_eq!(struggling.density, Density::Spacious);
        let heavy = presentation_for(LearnerSignals { heavy_scaffolding: true, ..Default::default() }, Momentum::Rising);
        assert_eq!((heavy.density, heavy.emphasis), (Density::Spacious, Emphasis::Visual), "heavy scaffolding beats a rising streak");
        let json = serde_json::to_value(calm).unwrap();
        assert_eq!(json, serde_json::json!({"density": "comfortable", "emphasis": "balanced", "layout": "stacked"}));
    }

    #[test]
    fn dominant_practice_type_is_never_planned_as_a_normal_gate() {
        let after_concept = [theory(), gate(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed)];
        // Without dominant: plans HandsOnMission by default.
        let p = plan_with(&after_concept, |_| {});
        assert_eq!(p.block_type, Some(DynamicBlockType::HandsOnMission));

        // When HandsOnMission dominates prior classes (>50%): plans HeuristicErrorAudit instead!
        let p_avoid = plan_with(&after_concept, |i| {
            i.dominant = &[DynamicBlockType::HandsOnMission];
        });
        assert_eq!(p_avoid.block_type, Some(DynamicBlockType::HeuristicErrorAudit));
        assert_ne!(p_avoid.block_type, Some(DynamicBlockType::HandsOnMission));
    }
}
