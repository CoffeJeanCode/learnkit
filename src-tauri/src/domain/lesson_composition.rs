use serde::{Deserialize, Serialize};

use crate::domain::notebook::DynamicBlockType;

/// The ~5 epistemological/disciplinary profiles `agents::block_generator_agent`'s
/// system prompt ("COMPOSICIÓN DINÁMICA" section) describes in prose to pick a
/// STARTING disposition for a class's block sequence — never a rigid mold (a
/// class can, and often does, legitimately deviate from its profile's
/// suggested chain). This enum and `suggested_sequence` exist purely to make
/// that same information ALSO exist as inspectable, unit-testable data: before
/// this module, the only place the profile-to-sequence mapping lived was as
/// natural-language text inside the LLM prompt, which meant nobody could write
/// a test asserting e.g. "the debugging profile suggests `heuristic_error_audit`
/// before `hands_on_mission`". This table does not replace the prompt prose
/// (the model still needs the natural-language version) and is not wired into
/// any runtime rejection/guardrail — it's a parallel, testable source of truth
/// for what the prompt already says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisciplineProfile {
    /// "Fenómenos espaciales o biológicos" (anatomía, mecánica, campos,
    /// procesos naturales con estructura física real).
    SpatialBiological,
    /// "Razonamiento cuantitativo, cálculo u optimización".
    QuantitativeMath,
    /// "Procesos cíclicos, balances o flujos de sistemas".
    CyclicSystemic,
    /// "Arquitectura de software, datos o depuración".
    SoftwareDebugging,
    /// "Liderazgo, toma de decisiones o ciencias sociales" (el contenido ES
    /// una decisión con consecuencias, no un concepto o procedimiento
    /// técnico).
    LeadershipDecisions,
}

impl DisciplineProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SpatialBiological => "spatial_biological",
            Self::QuantitativeMath => "quantitative_math",
            Self::CyclicSystemic => "cyclic_systemic",
            Self::SoftwareDebugging => "software_debugging",
            Self::LeadershipDecisions => "leadership_decisions",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "spatial_biological" => Some(Self::SpatialBiological),
            "quantitative_math" => Some(Self::QuantitativeMath),
            "cyclic_systemic" => Some(Self::CyclicSystemic),
            "software_debugging" => Some(Self::SoftwareDebugging),
            "leadership_decisions" => Some(Self::LeadershipDecisions),
            _ => None,
        }
    }

    /// All 5 profiles, in the same order the prompt lists them — used by
    /// tests that want to sweep every profile rather than name them one by
    /// one.
    pub const ALL: [DisciplineProfile; 5] = [
        Self::SpatialBiological,
        Self::QuantitativeMath,
        Self::CyclicSystemic,
        Self::SoftwareDebugging,
        Self::LeadershipDecisions,
    ];
}

/// The suggested block chain for `profile`, transcribed verbatim from the
/// "COMPOSICIÓN DINÁMICA" section of `agents::block_generator_agent`'s system
/// prompt. This is a STARTING disposition, not a mold: the prompt explicitly
/// tells the model these are "puntos de partida, no una tabla que copias", so
/// nothing in this crate treats a deviation from this sequence as invalid.
///
/// Two things the prompt itself calls out that this table intentionally does
/// NOT capture as static data, to avoid silently changing behavior or
/// encoding a contradiction:
/// - `SoftwareDebugging`'s prose also allows an OPTIONAL second
///   `hands_on_mission` ("transferencia lejana") before the closing block,
///   which this table omits — the base 4-block chain below is the sequence
///   actually asserted by this module's tests.
/// - `QuantitativeMath`'s prose does not explicitly mention
///   `metacognitive_closure` in its chain (every other profile's prose does),
///   even though the prompt separately states the closing block is mandatory
///   for every class once mastery is reached. This table transcribes the
///   chain exactly as written rather than inventing the omitted step.
pub fn suggested_sequence(profile: DisciplineProfile) -> &'static [DynamicBlockType] {
    use DynamicBlockType::*;

    match profile {
        DisciplineProfile::SpatialBiological => &[
            AnchoredMicroTheory,
            DeclarativeVisualDiagram,
            InteractivePredictionGate,
            DeclarativeVisualDiagram,
            MetacognitiveClosure,
        ],
        DisciplineProfile::QuantitativeMath => {
            &[AnchoredMicroTheory, InteractivePredictionGate, HeuristicErrorAudit, HandsOnMission]
        }
        DisciplineProfile::CyclicSystemic => {
            &[AnchoredMicroTheory, DeclarativeVisualDiagram, HeuristicErrorAudit, MetacognitiveClosure]
        }
        DisciplineProfile::SoftwareDebugging => {
            &[HeuristicErrorAudit, InteractivePredictionGate, HandsOnMission, MetacognitiveClosure]
        }
        DisciplineProfile::LeadershipDecisions => {
            &[AnchoredMicroTheory, BranchingScenarioChallenge, HandsOnMission, MetacognitiveClosure]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::DynamicBlockType;

    fn index_of(seq: &[DynamicBlockType], t: DynamicBlockType) -> Option<usize> {
        seq.iter().position(|&b| b == t)
    }

    #[test]
    fn every_profile_round_trips_through_as_str_and_parse() {
        for profile in DisciplineProfile::ALL {
            assert_eq!(DisciplineProfile::parse(profile.as_str()), Some(profile));
        }
        assert_eq!(DisciplineProfile::parse("not_a_real_profile"), None);
    }

    #[test]
    fn every_profile_sequence_is_within_3_to_5_blocks() {
        for profile in DisciplineProfile::ALL {
            let seq = suggested_sequence(profile);
            assert!(
                (3..=5).contains(&seq.len()),
                "{profile:?} sequence has {} blocks, expected 3-5",
                seq.len()
            );
        }
    }

    #[test]
    fn no_profile_repeats_a_block_type_consecutively() {
        for profile in DisciplineProfile::ALL {
            let seq = suggested_sequence(profile);
            for pair in seq.windows(2) {
                assert_ne!(pair[0], pair[1], "{profile:?} repeats {:?} consecutively", pair[0]);
            }
        }
    }

    #[test]
    fn spatial_biological_pairs_theory_with_its_own_diagram_then_gates_before_closing() {
        let seq = suggested_sequence(DisciplineProfile::SpatialBiological);
        assert_eq!(seq.first(), Some(&DynamicBlockType::AnchoredMicroTheory));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
        assert!(index_of(seq, DynamicBlockType::AnchoredMicroTheory) < index_of(seq, DynamicBlockType::InteractivePredictionGate));
    }

    #[test]
    fn quantitative_math_builds_a_worked_example_toward_hands_on_practice() {
        let seq = suggested_sequence(DisciplineProfile::QuantitativeMath);
        assert_eq!(seq.first(), Some(&DynamicBlockType::AnchoredMicroTheory));
        assert_eq!(seq.last(), Some(&DynamicBlockType::HandsOnMission));
        assert!(index_of(seq, DynamicBlockType::InteractivePredictionGate) < index_of(seq, DynamicBlockType::HeuristicErrorAudit));
    }

    #[test]
    fn cyclic_systemic_maps_causal_chain_before_auditing_bottlenecks() {
        let seq = suggested_sequence(DisciplineProfile::CyclicSystemic);
        assert!(index_of(seq, DynamicBlockType::DeclarativeVisualDiagram) < index_of(seq, DynamicBlockType::HeuristicErrorAudit));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn software_debugging_audits_the_bug_before_the_hands_on_mission() {
        let seq = suggested_sequence(DisciplineProfile::SoftwareDebugging);
        assert_eq!(seq.first(), Some(&DynamicBlockType::HeuristicErrorAudit));
        assert!(index_of(seq, DynamicBlockType::HeuristicErrorAudit) < index_of(seq, DynamicBlockType::HandsOnMission));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn leadership_decisions_frames_the_branch_before_justifying_the_choice() {
        let seq = suggested_sequence(DisciplineProfile::LeadershipDecisions);
        assert!(index_of(seq, DynamicBlockType::BranchingScenarioChallenge) < index_of(seq, DynamicBlockType::HandsOnMission));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }
}
