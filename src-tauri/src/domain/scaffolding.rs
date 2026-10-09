//! Deterministic scaffolding plan for the NEXT block of a class.
//!
//! A real student reported that knowledge jumped abruptly between blocks:
//! a gate would test something the previous blocks only brushed against, or
//! a second theory block would stack a new concept on an unchecked one. The
//! prompt alone ("one idea per block") cannot enforce a gradient, so the
//! ladder is computed here from what the notebook already holds
//! (`blocksSoFar` + student outcomes) and handed to the generator and the
//! critic as `scaffolding` — the same way `masteryStatus` is.
//!
//! Grounded in the personalization/learning-science principles of
//! "Towards an AI-Augmented Textbook" (arXiv:2509.13348): prior-knowledge
//! activation, cognitive-load control (one idea, examples before abstraction),
//! formative checks right after a segment, a challenge level that is neither
//! boring nor frustrating, and adapting to the learner's gaps.

use crate::domain::notebook::{BlockStatus, DynamicBlockType, GateCategory, GeneratedSectionBlock, NotebookBlock};

/// How much help the next block must give. Ordered from most to least support;
/// the ladder only ever fades ONE rung per step (and steps back on struggle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportLevel {
    /// First contact: worked example, concrete analogy, zero technical terms
    /// before consolidation, nothing for the student to produce yet.
    Full,
    /// Taught but not yet checked: a low-stakes check answerable ONLY from
    /// what was already taught, with hints.
    Guided,
    /// Conceptual gate passed: apply the idea to a NEW case, hints on demand.
    Faded,
    /// Both gates passed: transfer to a different context, no hints.
    Independent,
}

impl SupportLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Guided => "guided",
            Self::Faded => "faded",
            Self::Independent => "independent",
        }
    }

    fn guidance(self) -> &'static str {
        match self {
            Self::Full => {
                "apoyo_total: primer contacto con la idea. Ejemplo resuelto o analogía concreta; \
                 nada que el estudiante deba producir todavía; terminología técnica solo después de \
                 anclarla en lo cotidiano."
            }
            Self::Guided => {
                "apoyo_guiado: lo enseñado ya está en contentSoFar. Pide UN paso pequeño que se \
                 responda SOLO con lo ya explicado, con pistas; si algo no está en contentSoFar, \
                 enséñalo antes en vez de evaluarlo."
            }
            Self::Faded => {
                "apoyo_atenuado: la comprensión ya fue demostrada. Aplica la misma idea a un caso \
                 NUEVO de dificultad apenas mayor, con pistas solo a demanda (menos que antes)."
            }
            Self::Independent => {
                "apoyo_minimo: ambas dimensiones demostradas. Transferencia a un contexto distinto, \
                 sin pistas; el reto es adaptar la regla, no recordarla."
            }
        }
    }

    /// One rung MORE support (used after a failed attempt).
    fn step_back(self) -> Self {
        match self {
            Self::Independent => Self::Faded,
            Self::Faded => Self::Guided,
            Self::Guided | Self::Full => Self::Full,
        }
    }
}

/// Challenge-skill balance read from how the student actually performed on
/// resolved gates. The paper's motivation rubric asks for "an optimal level
/// of challenge, between too easy (boring) and too hard (frustrating)":
/// first-try streaks mean the next block is too easy, escalations mean it was
/// too hard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Momentum {
    /// Last 2 resolved gates passed on the first attempt → raise the challenge.
    Rising,
    Steady,
    /// Last resolved gate was escalated or needed 3+ attempts → lower it.
    Struggling,
}

impl Momentum {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Steady => "steady",
            Self::Struggling => "struggling",
        }
    }

    fn guidance(self) -> &'static str {
        match self {
            Self::Rising => {
                "ritmo_alto: las últimas compuertas salieron al primer intento — el reto anterior fue \
                 demasiado fácil. Sube la dificultad UN grado (caso con una complicación, dato \
                 faltante o dos variables) y reconoce el avance con una frase concreta, sin adular."
            }
            Self::Steady => {
                "ritmo_estable: mantén el reto apenas por encima de lo ya demostrado — que exija \
                 pensar, pero que el estudiante pueda resolverlo con lo que ya vio."
            }
            Self::Struggling => {
                "ritmo_bajo: la última compuerta costó demasiado. Baja el reto: caso más corto y \
                 concreto, una sola variable, un logro pequeño y alcanzable. Tono de ánimo \
                 específico (qué sí logró), nunca condescendiente."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldingPlan {
    /// 1-based position of the block about to be generated.
    pub step_number: usize,
    pub support_level: SupportLevel,
    /// Theory/diagram blocks already shown (the only things a gate may test).
    pub teaching_blocks: usize,
    /// The previous block taught something and nothing has checked it yet.
    pub last_taught_unchecked: bool,
    /// The previous gate was failed — the ladder stepped back one rung.
    pub stepped_back: bool,
    pub momentum: Momentum,
    pub gates_passed: usize,
    /// Both mastery gates are passed and no transfer challenge exists yet:
    /// the next block must be the transfer mission (`isTransfer`), not the
    /// closure.
    pub transfer_due: bool,
}

/// A transfer mission already exists in the class (attempted, whatever its
/// outcome — an escalated transfer must not block the closure forever).
pub fn transfer_attempted(blocks: &[NotebookBlock]) -> bool {
    blocks.iter().any(|b| b.block_type == DynamicBlockType::HandsOnMission && b.content_json.get("isTransfer").and_then(|v| v.as_bool()) == Some(true))
}

/// Deterministic rules for the transfer challenge, checked on every candidate
/// block (see `generation::critique`):
/// - `isTransfer` only once BOTH mastery gates are passed (it is applied
///   knowledge, not first contact);
/// - its constraints must differ from every earlier mission's (a NEW case);
/// - the closure cannot arrive before a transfer was attempted (forced and
///   regenerated closures are exempt: `exempt`).
pub fn transfer_violations(candidate: &GeneratedSectionBlock, existing: &[NotebookBlock], mastery_complete: bool, exempt: bool) -> Vec<String> {
    let mut v = Vec::new();
    match candidate {
        GeneratedSectionBlock::HandsOnMission { is_transfer: true, constraints, .. } => {
            if !mastery_complete {
                v.push("isTransfer solo es válido cuando el estudiante ya superó la compuerta conceptual Y la de práctica".to_string());
            }
            let prior: Vec<String> = existing
                .iter()
                .filter(|b| b.block_type == DynamicBlockType::HandsOnMission)
                .filter_map(|b| b.content_json.get("constraints").and_then(|c| c.as_array()))
                .flatten()
                .filter_map(|c| c.as_str().map(|s| s.trim().to_lowercase()))
                .collect();
            if let Some(repeated) = constraints.iter().find(|c| prior.contains(&c.trim().to_lowercase())) {
                v.push(format!("el reto de transferencia repite la restricción «{repeated}» de una misión anterior — debe ser un caso NUEVO con restricciones distintas"));
            }
        }
        GeneratedSectionBlock::MetacognitiveClosure { .. } if !exempt && mastery_complete && !transfer_attempted(existing) => {
            v.push("antes del cierre falta el reto de transferencia: genera una hands_on_mission con isTransfer=true (caso nuevo, restricciones distintas, scaffoldingHints vacío)".to_string());
        }
        _ => {}
    }
    v
}

fn momentum_of(blocks: &[NotebookBlock]) -> Momentum {
    let resolved: Vec<&NotebookBlock> =
        blocks.iter().filter(|b| b.block_type.is_gate() && matches!(b.status, BlockStatus::Passed | BlockStatus::Escalated)).collect();
    match resolved.as_slice() {
        [.., last] if last.status == BlockStatus::Escalated || last.attempt_count >= 3 => Momentum::Struggling,
        [.., a, b] if a.status == BlockStatus::Passed && b.status == BlockStatus::Passed && a.attempt_count <= 1 && b.attempt_count <= 1 => {
            Momentum::Rising
        }
        _ => Momentum::Steady,
    }
}

fn is_teaching(t: DynamicBlockType) -> bool {
    matches!(t, DynamicBlockType::AnchoredMicroTheory | DynamicBlockType::DeclarativeVisualDiagram)
}

/// Short human label of what a block covered — the "bridge" the next block
/// must start from. Falls back through the per-type headline fields.
fn headline(block: &NotebookBlock) -> Option<String> {
    ["title", "question", "challengeStatement", "scenario", "instruction", "synthesisTask"]
        .iter()
        .find_map(|k| block.content_json.get(*k).and_then(|v| v.as_str()))
        .map(|s| s.chars().take(120).collect())
}

/// Computes the ladder rung for the block that comes after `blocks`.
/// `needs_heavy_scaffolding` mirrors `LearnerCognitiveMemory::needs_heavy_
/// scaffolding`: such a student never gets less than guided support until a
/// conceptual gate has actually been passed.
pub fn plan(blocks: &[NotebookBlock], needs_heavy_scaffolding: bool) -> ScaffoldingPlan {
    let passed = |cat: GateCategory| {
        blocks.iter().any(|b| b.status == BlockStatus::Passed && b.block_type.gate_category() == Some(cat))
    };
    let teaching_blocks = blocks.iter().filter(|b| is_teaching(b.block_type)).count();

    let mut level = if !passed(GateCategory::Conceptual) {
        if teaching_blocks == 0 || needs_heavy_scaffolding {
            SupportLevel::Full
        } else {
            SupportLevel::Guided
        }
    } else if !passed(GateCategory::Practice) {
        SupportLevel::Faded
    } else {
        SupportLevel::Independent
    };
    // A heavy-scaffolding student who has taught content in hand is ready for
    // a guided check, not another round of pure exposition.
    if needs_heavy_scaffolding && teaching_blocks > 0 && level == SupportLevel::Full && blocks.last().is_some_and(|b| is_teaching(b.block_type)) {
        level = SupportLevel::Guided;
    }

    let last = blocks.last();
    let stepped_back = last.is_some_and(|b| b.block_type.is_gate() && b.attempt_count > 0 && b.status != BlockStatus::Passed);
    if stepped_back {
        level = level.step_back();
    }

    let transfer_due = passed(GateCategory::Conceptual) && passed(GateCategory::Practice) && !transfer_attempted(blocks);
    ScaffoldingPlan {
        step_number: blocks.len() + 1,
        support_level: level,
        teaching_blocks,
        last_taught_unchecked: last.is_some_and(|b| is_teaching(b.block_type)),
        stepped_back,
        momentum: momentum_of(blocks),
        gates_passed: blocks.iter().filter(|b| b.block_type.is_gate() && b.status == BlockStatus::Passed).count(),
        transfer_due,
    }
}

impl ScaffoldingPlan {
    /// The `scaffolding` object fed to the generator and the critic.
    pub fn to_json(&self, blocks: &[NotebookBlock]) -> serde_json::Value {
        let covered: Vec<String> = blocks.iter().filter_map(headline).collect();
        let mut out = serde_json::json!({
            "stepNumber": self.step_number,
            "supportLevel": self.support_level.as_str(),
            "supportGuidance": self.support_level.guidance(),
            "teachingBlocksSoFar": self.teaching_blocks,
            "challengeBalance": {
                "momentum": self.momentum.as_str(),
                "guidance": self.momentum.guidance(),
            },
            "gatesPassedSoFar": self.gates_passed,
            "alreadyCovered": covered,
            "rules": "Un solo paso nuevo respecto al bloque anterior. Abre conectando con \
                      bridgeFrom. Una compuerta solo evalúa lo ya enseñado en contentSoFar.",
        });
        if let Some(last) = blocks.last() {
            out["bridgeFrom"] = serde_json::json!({
                "blockType": last.block_type.as_str(),
                "headline": headline(last),
            });
        }
        if self.last_taught_unchecked {
            out["nextShouldCheck"] = serde_json::Value::String(
                "El bloque anterior enseñó algo que nadie ha comprobado: prefiere una compuerta/chequeo \
                 sobre ESO antes de añadir teoría nueva."
                    .to_string(),
            );
        }
        if self.transfer_due {
            out["transferDue"] = serde_json::Value::String(
                "Ambas dimensiones de maestría están superadas: el SIGUIENTE bloque debe ser la hands_on_mission de \
                 TRANSFERENCIA (isTransfer=true): el mismo concepto en un caso NUEVO, restricciones distintas a las \
                 de las misiones anteriores, scaffoldingHints vacío, nivel de reto apenas mayor que lo ya demostrado. \
                 Solo después puede venir el cierre."
                    .to_string(),
            );
        }
        if self.stepped_back {
            out["steppedBack"] = serde_json::Value::String(
                "El estudiante falló la compuerta anterior: retrocede un escalón — más apoyo, mismo \
                 concepto, sin material nuevo."
                    .to_string(),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(i: u32, t: DynamicBlockType, status: BlockStatus, attempts: u32) -> NotebookBlock {
        NotebookBlock {
            id: format!("b{i}"),
            document_id: "d".into(),
            block_type: t,
            content_json: serde_json::json!({ "title": format!("t{i}") }),
            order_index: i,
            status,
            attempt_count: attempts,
            last_feedback: None,
        }
    }

    use BlockStatus::*;
    use DynamicBlockType::*;

    #[test]
    fn first_block_is_full_support() {
        let p = plan(&[], false);
        assert_eq!((p.step_number, p.support_level), (1, SupportLevel::Full));
    }

    #[test]
    fn taught_but_unchecked_asks_for_a_check() {
        let p = plan(&[block(0, AnchoredMicroTheory, Ready, 0)], false);
        assert_eq!(p.support_level, SupportLevel::Guided);
        assert!(p.last_taught_unchecked);
    }

    #[test]
    fn support_fades_one_rung_per_passed_dimension() {
        let b = [block(0, AnchoredMicroTheory, Ready, 0), block(1, InteractivePredictionGate, Passed, 1)];
        assert_eq!(plan(&b, false).support_level, SupportLevel::Faded);
        let b = [
            block(0, AnchoredMicroTheory, Ready, 0),
            block(1, InteractivePredictionGate, Passed, 1),
            block(2, HandsOnMission, Passed, 1),
        ];
        assert_eq!(plan(&b, false).support_level, SupportLevel::Independent);
    }

    #[test]
    fn failed_gate_steps_back_one_rung() {
        let b = [block(0, AnchoredMicroTheory, Ready, 0), block(1, InteractivePredictionGate, Failed, 1)];
        let p = plan(&b, false);
        assert!(p.stepped_back);
        assert_eq!(p.support_level, SupportLevel::Full);
    }

    #[test]
    fn heavy_scaffolding_never_skips_ahead_before_a_conceptual_pass() {
        let p = plan(&[block(0, AnchoredMicroTheory, Ready, 0), block(1, DeclarativeVisualDiagram, Ready, 0)], true);
        assert_eq!(p.support_level, SupportLevel::Guided);
        let p = plan(&[], true);
        assert_eq!(p.support_level, SupportLevel::Full);
    }

    fn gate(i: u32, status: BlockStatus, attempts: u32) -> NotebookBlock {
        block(i, InteractivePredictionGate, status, attempts)
    }

    #[test]
    fn two_first_try_passes_raise_the_challenge() {
        let b = [gate(0, Passed, 1), gate(1, Passed, 1)];
        assert_eq!(plan(&b, false).momentum, Momentum::Rising);
        let b = [gate(0, Passed, 2), gate(1, Passed, 1)];
        assert_eq!(plan(&b, false).momentum, Momentum::Steady);
    }

    #[test]
    fn an_escalation_or_a_3rd_attempt_pass_lowers_the_challenge() {
        assert_eq!(plan(&[gate(0, Escalated, 3)], false).momentum, Momentum::Struggling);
        assert_eq!(plan(&[gate(0, Passed, 3)], false).momentum, Momentum::Struggling);
        assert_eq!(plan(&[], false).momentum, Momentum::Steady);
    }

    #[test]
    fn json_carries_bridge_and_covered_headlines() {
        let b = [block(0, AnchoredMicroTheory, Ready, 0)];
        let j = plan(&b, false).to_json(&b);
        assert_eq!(j["bridgeFrom"]["headline"], "t0");
        assert_eq!(j["alreadyCovered"][0], "t0");
        assert!(j.get("nextShouldCheck").is_some());
    }

    fn mission(constraints: &[&str], transfer: bool, hints: &[&str]) -> GeneratedSectionBlock {
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "c".into(),
            expected_milestone_artifact: "a".into(),
            constraints: constraints.iter().map(|s| s.to_string()).collect(),
            scaffolding_hints: hints.iter().map(|s| s.to_string()).collect(),
            evaluation_rubric_summary: vec!["r".into()],
            is_transfer: transfer,
            visual_aid: None,
        }
    }

    fn mastered() -> Vec<NotebookBlock> {
        vec![block(0, InteractivePredictionGate, Passed, 1), {
            let mut m = block(1, HandsOnMission, Passed, 1);
            m.content_json = serde_json::json!({ "constraints": ["Sin librerías externas"] });
            m
        }]
    }

    #[test]
    fn transfer_is_due_once_both_gates_are_passed_until_one_is_attempted() {
        assert!(!plan(&[block(0, InteractivePredictionGate, Passed, 1)], false).transfer_due, "practice not passed yet");
        assert!(plan(&mastered(), false).transfer_due);
        assert!(plan(&mastered(), false).to_json(&mastered()).get("transferDue").is_some());

        let mut with_transfer = mastered();
        let mut t = block(2, HandsOnMission, Escalated, 3);
        t.content_json = serde_json::json!({ "isTransfer": true });
        with_transfer.push(t);
        assert!(!plan(&with_transfer, false).transfer_due, "an escalated transfer must not block the closure forever");
    }

    #[test]
    fn closure_is_rejected_until_a_transfer_was_attempted_unless_exempt() {
        let closure = GeneratedSectionBlock::MetacognitiveClosure {
            synthesis_task: "s".into(),
            prediction_comparison: crate::domain::notebook::PredictionComparison {
                initial_prediction: "i".into(),
                final_result: "f".into(),
                contrast_narrative: "c".into(),
            },
            self_evaluation_checklist: vec!["x".into()],
        };
        assert_eq!(transfer_violations(&closure, &mastered(), true, false).len(), 1);
        assert!(transfer_violations(&closure, &mastered(), true, true).is_empty(), "forced/regenerated closures are exempt");
        assert!(transfer_violations(&closure, &mastered(), false, false).is_empty(), "mastery rule is grounding's job");
    }

    #[test]
    fn a_transfer_mission_needs_mastery_and_constraints_unseen_in_earlier_missions() {
        let fresh = mission(&["Datos con valores nulos"], true, &[]);
        assert!(transfer_violations(&fresh, &mastered(), true, false).is_empty());
        assert_eq!(transfer_violations(&fresh, &mastered(), false, false).len(), 1, "too early");

        let repeated = mission(&["  sin librerías EXTERNAS "], true, &[]);
        assert_eq!(transfer_violations(&repeated, &mastered(), true, false).len(), 1, "same constraint, different case it is not");
        assert!(transfer_violations(&mission(&["x"], false, &["pista"]), &mastered(), true, false).is_empty(), "ordinary missions untouched");
    }
}

