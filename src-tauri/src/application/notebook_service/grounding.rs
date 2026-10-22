//! Content-level grounding for ONE freshly generated block — the incremental
//! successor to the retired whole-notebook `notebook_grounding_violations`
//! (which judged an entire `GeneratedDynamicNotebook` at once). Now that
//! `notebook_agent` is called once per block (see `generation`), the same
//! rules — no two adjacent blocks of the same type, 3-5 total, per-block-type
//! field completeness, course-wide practice-family dominance — are checked
//! at EVERY block call instead of once at the end.

use std::collections::HashMap;

use crate::domain::notebook::{
    BlockStatus, DynamicBlockType, GeneratedSectionBlock, InteractionMode, MermaidChartType, MissionSupportLevel, NotebookBlock,
    StaticVisualSpec,
};

/// The interchangeable "practice/activity" family — these three all serve
/// the same pedagogical ROLE (give the student something to DO with the
/// material) and are the ones a real model was observed defaulting to the
/// same member of, regardless of topic. Deliberately excludes
/// `anchored_micro_theory`/`declarative_visual_diagram`/
/// `interactive_prediction_gate`/`metacognitive_closure`: those serve fixed
/// STRUCTURAL roles that are SUPPOSED to recur across most classes.
const PRACTICE_BLOCK_FAMILY: [&str; 3] = ["heuristic_error_audit", "hands_on_mission", "branching_scenario_challenge"];

/// Own word ceiling for `anchored_micro_theory`'s `systemRule` (Capa 2, the
/// causal/mechanism layer) — widened from the old 160-words-combined cap,
/// then from 180, specifically so this layer alone can be exhaustive about
/// the mechanism as a numbered micro-steps sequence with a concrete
/// input/output example (see the prompt in `agents::block_generator_agent`),
/// independent of how terse the other 3 layers stay.
const MAX_SYSTEM_RULE_WORDS: usize = 240;

/// Combined word ceiling for `anchored_micro_theory`'s other 3 layers —
/// `intuitiveHook`, `analogyBoundary`, `frequentError` — kept terse even
/// though `systemRule` now has real room, so the block never degrades into
/// flat encyclopedic text.
const MAX_OTHER_LAYERS_WORDS: usize = 100;

/// Course-scoped variety check — judges the new block against the OTHER
/// classes already generated for the same course, not against this
/// notebook's own blocks. Only kicks in once there's real history to
/// compare against (2+ prior classes), and only for a type that ALREADY
/// dominates more than half of them.
pub(super) fn dominance_violation(
    block: &GeneratedSectionBlock,
    (usage, total_classes): &(HashMap<String, i64>, i64),
) -> Option<String> {
    if *total_classes < 2 {
        return None;
    }
    let bt = block.block_type().as_str();
    if !PRACTICE_BLOCK_FAMILY.contains(&bt) || matches!(block, GeneratedSectionBlock::HandsOnMission { is_transfer: true, .. }) {
        return None;
    }
    let prior = usage.get(bt).copied().unwrap_or(0);
    if (prior as f64) / (*total_classes as f64) > 0.5 {
        Some(format!(
            "blockType {bt} ya aparece en {prior} de {total_classes} clases anteriores de este curso — \
             varía el bloque de práctica en vez de repetirlo otra vez"
        ))
    } else {
        None
    }
}

/// Safety ceiling only — NOT a target length. The notebook is meant to keep
/// generating theory/practice cycles for as long as the student needs them
/// (see `MasteryProgress`); this only exists to guarantee the class
/// eventually ends even if the model or the student never converges. Kept
/// generous specifically so it's never the normal way a class finishes.
pub(super) const MAX_TOTAL_BLOCKS: usize = 20;

/// Mastery evidence accumulated so far in this notebook: whether the
/// student has PASSED at least one gate of EACH category (see
/// `DynamicBlockType::gate_category`). Computed by the caller
/// (`generation`) from the persisted blocks — this module only judges the
/// booleans, it doesn't need to know about `NotebookBlock`/`BlockStatus`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MasteryProgress {
    pub(super) has_passed_conceptual: bool,
    pub(super) has_passed_practice: bool,
}

impl MasteryProgress {
    pub(super) fn complete(self) -> bool {
        self.has_passed_conceptual && self.has_passed_practice
    }
}

/// Scans a document's persisted blocks for passed gates of each category.
pub(super) fn mastery_progress(blocks: &[NotebookBlock]) -> MasteryProgress {
    let mut progress = MasteryProgress::default();
    for b in blocks {
        if b.status != BlockStatus::Passed {
            continue;
        }
        match b.block_type.gate_category() {
            Some(crate::domain::notebook::GateCategory::Conceptual) => progress.has_passed_conceptual = true,
            Some(crate::domain::notebook::GateCategory::Practice) => progress.has_passed_practice = true,
            None => {}
        }
    }
    progress
}

/// `prior_types` is every block type already persisted in this document, in
/// order (empty for the very first block). `total_so_far` is the count
/// INCLUDING this new block. `is_escalation` exempts a re-approach block
/// (generated after 3 gate failures) from the safety ceiling — abandoning a
/// student mid-gate right at the ceiling would be worse than a small
/// overshoot. `force_close` is set once `total_so_far` itself hits the
/// ceiling on a NON-escalation call — the model is compelled to close
/// immediately regardless of mastery status (see
/// `generation::generate_and_persist_block`), which is the only situation
/// where a `metacognitive_closure` is grounded without full mastery.
pub(super) fn single_block_violations(
    prior_types: &[DynamicBlockType],
    block: &GeneratedSectionBlock,
    total_so_far: usize,
    is_escalation: bool,
    mastery: MasteryProgress,
    force_close: bool,
    has_due_retrieval: bool,
) -> Vec<String> {
    let mut v = Vec::new();
    if !is_escalation && total_so_far > MAX_TOTAL_BLOCKS {
        v.push(format!("la clase ya tiene {total_so_far} bloques, el tope de seguridad es {MAX_TOTAL_BLOCKS}"));
    }
    let is_transfer = matches!(block, GeneratedSectionBlock::HandsOnMission { is_transfer: true, .. });
    if let Some(last) = prior_types.last() {
        // A transfer mission may directly follow the mission that completed
        // mastery, and an escalation may answer a mission with a MORE
        // scaffolded mission (the student asked for an easier step).
        let eased_mission = is_escalation
            && matches!(
                block,
                GeneratedSectionBlock::HandsOnMission {
                    support_level: Some(MissionSupportLevel::Full | MissionSupportLevel::Guided | MissionSupportLevel::Faded),
                    ..
                }
            );
        if *last == block.block_type() && !is_transfer && !eased_mission {
            v.push(format!(
                "blockType {} se repite consecutivamente respecto al bloque anterior — la secuencia debe variar",
                block.block_type().as_str()
            ));
        }
    }
    v.extend(mission_support_violations(prior_types, block));
    let is_first_block = prior_types.is_empty();
    if is_first_block && has_due_retrieval && !is_escalation && block.block_type() != DynamicBlockType::SpacedInterleavedRetrieval {
        v.push(
            "el estudiante tiene conceptos vencidos para recuperación espaciada (learnerMemory.dueRetrieval) — el \
             PRIMER bloque de esta clase debe ser spaced_interleaved_retrieval antes de tocar material nuevo"
                .to_string(),
        );
    }
    if !is_first_block && block.block_type() == DynamicBlockType::SpacedInterleavedRetrieval {
        v.push(
            "spaced_interleaved_retrieval solo es válido como el PRIMER bloque de la clase, nunca más adelante"
                .to_string(),
        );
    }
    // Golden progression, POSITIONAL: the first interactive block after the
    // class's first theory must practice the basic correct case, never audit
    // errors the student has never practiced. Deliberately NOT conditioned on
    // the learner profile: the diagnostic profile is often absent now that
    // batteries are never synthesized (see `roadmap_service::synth`), and the
    // real-world failure happened with no usable profile at all. Exempt when
    // `is_escalation` is set — the call site FUSES `is_escalation ||
    // is_regeneration` into this one flag (see `generation::critique`), and
    // regeneration repairs a block IN PLACE: a pre-existing audit at this
    // position would otherwise be permanently unrepairable (required type ==
    // audit vs this rule). A `hands_on_mission` in the same position is
    // already covered by `mission_support_violations` (guided|faded plus a
    // template/steps required after teaching) — no duplicate rule here.
    if !is_escalation
        && prior_types.len() == 1
        && prior_types[0] == DynamicBlockType::AnchoredMicroTheory
        && block.block_type() == DynamicBlockType::HeuristicErrorAudit
    {
        v.push(
            "Para un principiante, el bloque inmediatamente posterior a la primera teoría debe practicar primero el caso \
             básico correcto (interactive_prediction_gate sobre el caso normal o práctica guiada paso a paso), nunca saltar \
             directo a auditar errores o casos borde sin práctica previa"
                .to_string(),
        );
    }
    if force_close && block.block_type() != DynamicBlockType::MetacognitiveClosure {
        v.push(
            "se alcanzó el tope de seguridad de bloques y se pidió el cierre inmediato, pero no generaste metacognitive_closure"
                .to_string(),
        );
    }
    if block.block_type() == DynamicBlockType::MetacognitiveClosure && !force_close && !mastery.complete() {
        let missing: Vec<&str> = [
            (!mastery.has_passed_conceptual).then_some("una compuerta conceptual (predicción o decisión)"),
            (!mastery.has_passed_practice).then_some("una compuerta de práctica (misión o auditoría)"),
        ]
        .into_iter()
        .flatten()
        .collect();
        v.push(format!(
            "metacognitive_closure llegó antes de tiempo — todavía falta que el estudiante supere {}",
            missing.join(" y ")
        ));
    }
    v.extend(block_field_violations(total_so_far.saturating_sub(1), block));
    v
}

/// Scaffolding monotonicity for `hands_on_mission`: theory must never jump
/// straight to an independent challenge. Judged from the prior block TYPES:
/// - a mission must declare its `supportLevel`;
/// - `independent` is reserved for the transfer mission (it needs both gates
///   passed, so it is never the first thing after teaching);
/// - the first practice after a teaching block (no gate in between) has to be
///   `guided`/`faded` — the bridge between "I read it" and "I do it alone";
/// - `guided`/`faded` missions must hand the student something to start from
///   (`workedTemplate` or `interactiveSteps`), never a blank page.
fn mission_support_violations(prior_types: &[DynamicBlockType], block: &GeneratedSectionBlock) -> Vec<String> {
    let GeneratedSectionBlock::HandsOnMission { support_level, is_transfer, worked_template, interactive_steps, .. } = block else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let Some(level) = support_level else {
        v.push("hands_on_mission: declara supportLevel (full | guided | faded | independent) según scaffolding.supportLevel".to_string());
        return v;
    };
    if *is_transfer {
        if *level != MissionSupportLevel::Independent {
            v.push("hands_on_mission de transferencia (isTransfer): supportLevel debe ser \"independent\"".to_string());
        }
        return v;
    }
    if *level == MissionSupportLevel::Independent {
        v.push(
            "hands_on_mission con supportLevel \"independent\" solo es válido como reto de transferencia (isTransfer=true, tras \
             superar ambas compuertas) — antes usa \"guided\" o \"faded\""
                .to_string(),
        );
    }
    let first_practice_after_teaching = prior_types
        .iter()
        .rposition(|t| matches!(t, DynamicBlockType::AnchoredMicroTheory | DynamicBlockType::DeclarativeVisualDiagram))
        .is_some_and(|idx| !prior_types[idx + 1..].iter().any(|t| t.is_gate()));
    if first_practice_after_teaching && !matches!(level, MissionSupportLevel::Guided | MissionSupportLevel::Faded) {
        v.push(
            "el primer bloque práctico tras la teoría/diagrama no puede ser una hands_on_mission sin andamiaje: debe ser \
             supportLevel \"guided\" o \"faded\" (o un bloque intermedio de chequeo antes)"
                .to_string(),
        );
    }
    if matches!(level, MissionSupportLevel::Guided | MissionSupportLevel::Faded) {
        let has_template = worked_template.as_deref().is_some_and(|t| !t.trim().is_empty());
        let has_steps = interactive_steps.as_ref().is_some_and(|s| !s.is_empty());
        if !has_template && !has_steps {
            v.push("hands_on_mission guided/faded: incluye workedTemplate (solución resuelta al ~70% con huecos ___ o TODO) y/o interactiveSteps — el estudiante no debe empezar frente a una hoja en blanco".to_string());
        }
    }
    v
}

fn block_field_violations(i: usize, b: &GeneratedSectionBlock) -> Vec<String> {
    let mut v = Vec::new();
    let empty = |field: &str| format!("bloque {i} ({}): {field} está vacío", b.block_type().as_str());

    match b {
        GeneratedSectionBlock::SpacedInterleavedRetrieval { items } => {
            if !(1..=crate::domain::learner_memory::MAX_RETRIEVAL_ITEMS_PER_BLOCK).contains(&items.len()) {
                v.push(format!(
                    "bloque {i} (spaced_interleaved_retrieval): tiene {} items, debe tener 1-{}",
                    items.len(),
                    crate::domain::learner_memory::MAX_RETRIEVAL_ITEMS_PER_BLOCK
                ));
            }
            for (idx, item) in items.iter().enumerate() {
                if item.concept_label.trim().is_empty() || item.prompt.trim().is_empty() || item.expected_answer.trim().is_empty() {
                    v.push(format!("bloque {i} (spaced_interleaved_retrieval): items[{idx}] tiene un campo vacío"));
                }
            }
        }
        GeneratedSectionBlock::AnchoredMicroTheory { title, intuitive_hook, analogy_boundary, system_rule, frequent_error } => {
            if title.trim().is_empty() {
                v.push(empty("title"));
            }
            if intuitive_hook.trim().is_empty() {
                v.push(empty("intuitiveHook"));
            }
            if system_rule.trim().is_empty() {
                v.push(empty("systemRule"));
            }
            if frequent_error.trim().is_empty() {
                v.push(empty("frequentError"));
            }
            let system_rule_words = system_rule.split_whitespace().count();
            if system_rule_words > MAX_SYSTEM_RULE_WORDS {
                v.push(format!(
                    "bloque {i} (anchored_micro_theory): systemRule tiene {system_rule_words} palabras, máximo {MAX_SYSTEM_RULE_WORDS}"
                ));
            }
            let other_layers_words = [intuitive_hook.as_str(), analogy_boundary.as_deref().unwrap_or(""), frequent_error.as_str()]
                .iter()
                .map(|s| s.split_whitespace().count())
                .sum::<usize>();
            if other_layers_words > MAX_OTHER_LAYERS_WORDS {
                v.push(format!(
                    "bloque {i} (anchored_micro_theory): intuitiveHook + analogyBoundary + frequentError suman {other_layers_words} palabras, \
                     máximo {MAX_OTHER_LAYERS_WORDS} combinadas — deben quedarse terse, el detalle técnico vive en systemRule"
                ));
            }
        }
        GeneratedSectionBlock::DeclarativeVisualDiagram { title, visual_aid, guided_walkthrough } => {
            if title.trim().is_empty() {
                v.push(empty("title"));
            }
            v.extend(visual_violations(i, visual_aid));
            if guided_walkthrough.is_empty() {
                v.push(format!("bloque {i} (declarative_visual_diagram): guidedWalkthrough está vacío"));
            }
        }
        GeneratedSectionBlock::BranchingScenarioChallenge { scenario, decision_point, branches, visual_aid } => {
            if scenario.trim().is_empty() {
                v.push(empty("scenario"));
            }
            if decision_point.trim().is_empty() {
                v.push(empty("decisionPoint"));
            }
            if !(2..=4).contains(&branches.len()) {
                v.push(format!("bloque {i} (branching_scenario_challenge): branches tiene {} elementos, debe tener 2-4", branches.len()));
            }
            let mut optimal_count = 0;
            for branch in branches {
                if branch.choice.trim().is_empty() || branch.consequence.trim().is_empty() {
                    v.push(format!("bloque {i} (branching_scenario_challenge): una branch tiene choice o consequence vacío"));
                }
                if branch.is_optimal {
                    optimal_count += 1;
                }
            }
            if optimal_count != 1 {
                v.push(format!(
                    "bloque {i} (branching_scenario_challenge): {optimal_count} branches marcadas isOptimal, debe ser exactamente 1"
                ));
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(i, visual));
            }
        }
        GeneratedSectionBlock::HeuristicErrorAudit {
            instruction,
            flawed_representation,
            guiding_questions,
            model_solution,
            faulty_line_number,
            visual_aid,
        } => {
            if instruction.trim().is_empty() {
                v.push(empty("instruction"));
            }
            if flawed_representation.context.trim().is_empty() || flawed_representation.buggy_snippet_or_diagram.trim().is_empty() {
                v.push(format!("bloque {i} (heuristic_error_audit): flawedRepresentation incompleta"));
            }
            if guiding_questions.is_empty() {
                v.push(format!("bloque {i} (heuristic_error_audit): guidingQuestions está vacío"));
            }
            if model_solution.trim().is_empty() {
                v.push(empty("modelSolution"));
            }
            if let Some(line) = faulty_line_number {
                let lines = flawed_representation.buggy_snippet_or_diagram.lines().count() as u32;
                if *line == 0 || *line > lines {
                    v.push(format!(
                        "bloque {i} (heuristic_error_audit): faultyLineNumber {line} fuera de rango — buggySnippetOrDiagram tiene {lines} líneas (se cuenta desde 1)"
                    ));
                }
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(i, visual));
            }
        }
        GeneratedSectionBlock::InteractivePredictionGate {
            question,
            options,
            conceptual_feedback_map,
            correct_option,
            interaction_mode,
            shuffled_steps,
            correct_order,
            boundary_cases,
            visual_aid,
        } => {
            if question.trim().is_empty() {
                v.push(empty("question"));
            }
            match interaction_mode.unwrap_or(InteractionMode::MultipleChoice) {
                InteractionMode::MultipleChoice => {
                    if !(2..=4).contains(&options.len()) {
                        v.push(format!("bloque {i} (interactive_prediction_gate): options tiene {} elementos, debe tener 2-4", options.len()));
                    }
                    for opt in options {
                        if !conceptual_feedback_map.contains_key(opt) {
                            v.push(format!("bloque {i} (interactive_prediction_gate): falta conceptualFeedbackMap para la opción \"{opt}\""));
                        }
                    }
                    if !options.contains(correct_option) {
                        v.push(format!("bloque {i} (interactive_prediction_gate): correctOption \"{correct_option}\" no está en options"));
                    }
                }
                InteractionMode::CausalChainOrder => {
                    let shuffled = shuffled_steps.as_deref().unwrap_or(&[]);
                    let key = correct_order.as_deref().unwrap_or(&[]);
                    if !(3..=4).contains(&shuffled.len()) {
                        v.push(format!("bloque {i} (interactive_prediction_gate/causal_chain_order): shuffledSteps tiene {} eslabones, debe tener 3-4", shuffled.len()));
                    }
                    let mut a: Vec<&String> = shuffled.iter().collect();
                    let mut b: Vec<&String> = key.iter().collect();
                    a.sort();
                    b.sort();
                    if a != b {
                        v.push(format!("bloque {i} (interactive_prediction_gate/causal_chain_order): correctOrder debe contener EXACTAMENTE los mismos eslabones que shuffledSteps, en orden causal"));
                    } else if shuffled == key {
                        v.push(format!("bloque {i} (interactive_prediction_gate/causal_chain_order): shuffledSteps ya viene en el orden correcto — desordénalo"));
                    }
                    let mut seen = std::collections::HashSet::new();
                    if shuffled.iter().any(|s| s.trim().is_empty() || !seen.insert(s.trim().to_lowercase())) {
                        v.push(format!("bloque {i} (interactive_prediction_gate/causal_chain_order): shuffledSteps tiene eslabones vacíos o repetidos"));
                    }
                }
                InteractionMode::BoundaryCaseAudit => {
                    let cases = boundary_cases.as_deref().unwrap_or(&[]);
                    if !(2..=3).contains(&cases.len()) {
                        v.push(format!("bloque {i} (interactive_prediction_gate/boundary_case_audit): boundaryCases tiene {} casos, debe tener 2-3", cases.len()));
                    }
                    if cases.iter().any(|c| c.statement.trim().is_empty()) {
                        v.push(format!("bloque {i} (interactive_prediction_gate/boundary_case_audit): un caso tiene statement vacío"));
                    }
                    if !cases.iter().any(|c| c.follows_rule) || !cases.iter().any(|c| !c.follows_rule) {
                        v.push(format!("bloque {i} (interactive_prediction_gate/boundary_case_audit): debe haber al menos un caso que cumple la regla y uno que la rompe (contraste real)"));
                    }
                }
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(i, visual));
            }
        }
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement,
            expected_milestone_artifact,
            constraints,
            scaffolding_hints,
            evaluation_rubric_summary,
            is_transfer,
            worked_template,
            interactive_steps,
            visual_aid,
            ..
        } => {
            if challenge_statement.trim().is_empty() {
                v.push(empty("challengeStatement"));
            }
            if let Some(template) = worked_template.as_deref().filter(|t| !t.trim().is_empty()) {
                if !template.contains("___") && !template.to_uppercase().contains("TODO") {
                    v.push(format!("bloque {i} (hands_on_mission): workedTemplate no tiene huecos — márcalos con ___ o TODO para que el estudiante complete"));
                }
            }
            for (idx, step) in interactive_steps.iter().flatten().enumerate() {
                if step.prompt.trim().is_empty() || step.expected_token.trim().is_empty() {
                    v.push(format!("bloque {i} (hands_on_mission): interactiveSteps[{idx}] tiene prompt o expectedToken vacío"));
                }
                if let Some(options) = &step.options {
                    if !(2..=4).contains(&options.len()) || !options.contains(&step.expected_token) {
                        v.push(format!("bloque {i} (hands_on_mission): interactiveSteps[{idx}].options debe tener 2-4 elementos e incluir expectedToken"));
                    }
                }
            }
            if *is_transfer && (worked_template.is_some() || interactive_steps.as_ref().is_some_and(|s| !s.is_empty())) {
                v.push(format!("bloque {i} (hands_on_mission): una misión de transferencia no lleva workedTemplate ni interactiveSteps — se evalúa sin apoyo"));
            }
            if expected_milestone_artifact.trim().is_empty() {
                v.push(empty("expectedMilestoneArtifact"));
            }
            if constraints.is_empty() {
                v.push(format!("bloque {i} (hands_on_mission): constraints está vacío"));
            }
            if scaffolding_hints.is_empty() && !is_transfer {
                v.push(format!("bloque {i} (hands_on_mission): scaffoldingHints está vacío"));
            }
            if evaluation_rubric_summary.is_empty() {
                v.push(format!("bloque {i} (hands_on_mission): evaluationRubricSummary está vacío"));
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(i, visual));
            }
        }
        GeneratedSectionBlock::MetacognitiveClosure { synthesis_task, prediction_comparison, self_evaluation_checklist } => {
            if synthesis_task.trim().is_empty() {
                v.push(empty("synthesisTask"));
            }
            if prediction_comparison.initial_prediction.trim().is_empty()
                || prediction_comparison.final_result.trim().is_empty()
                || prediction_comparison.contrast_narrative.trim().is_empty()
            {
                v.push(format!("bloque {i} (metacognitive_closure): predictionComparison incompleta"));
            }
            if self_evaluation_checklist.is_empty() {
                v.push(format!("bloque {i} (metacognitive_closure): selfEvaluationChecklist está vacío"));
            }
        }
    }
    v
}

fn visual_violations(i: usize, visual: &StaticVisualSpec) -> Vec<String> {
    let mut v = Vec::new();
    match visual {
        StaticVisualSpec::Mermaid { code, caption, chart_type } => {
            if code.trim().is_empty() {
                v.push(format!("bloque {i}: visualAid mermaid sin code"));
            }
            if caption.trim().is_empty() {
                v.push(format!("bloque {i}: visualAid mermaid sin caption"));
            }
            // `stateDiagram` is the one chart type whose grammar has no `::`
            // token: a Rust/C++-style path in a transition label fails to
            // parse in the browser (flowchart/sequenceDiagram accept it), so
            // the diagram renders as an error instead of a picture. Rejecting
            // here makes the model rewrite the label in the same turn.
            if *chart_type == MermaidChartType::StateDiagram && code.contains("::") {
                v.push(format!(
                    "bloque {i}: una etiqueta del stateDiagram lleva '::' (p. ej. String::from) y Mermaid no puede \
                     parsearla — reescríbela sin '::', p. ej. String desde \"nota\" en lugar de String::from(\"nota\")"
                ));
            }
        }
        StaticVisualSpec::DeclarativeSvg { elements, groups, caption, .. } => {
            if elements.is_empty() && groups.is_empty() {
                v.push(format!("bloque {i}: visualAid declarative_svg sin elements ni groups"));
            }
            if caption.trim().is_empty() {
                v.push(format!("bloque {i}: visualAid declarative_svg sin caption"));
            }
        }
        StaticVisualSpec::ConceptualMatrix { headers, rows, contrast_focus } => {
            if headers.is_empty() || rows.is_empty() {
                v.push(format!("bloque {i}: visualAid conceptual_matrix sin headers/rows"));
            }
            if contrast_focus.trim().is_empty() {
                v.push(format!("bloque {i}: visualAid conceptual_matrix sin contrastFocus"));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::PredictionComparison;

    fn valid_gate() -> GeneratedSectionBlock {
        GeneratedSectionBlock::InteractivePredictionGate {
            question: "q".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
            conceptual_feedback_map: [("A".to_string(), "a".to_string()), ("B".to_string(), "b".to_string())].into_iter().collect(),
            correct_option: "B".to_string(),
            interaction_mode: None,
            shuffled_steps: None,
            correct_order: None,
            boundary_cases: None,
            visual_aid: None,
        }
    }

    fn no_mastery() -> MasteryProgress {
        MasteryProgress::default()
    }

    fn full_mastery() -> MasteryProgress {
        MasteryProgress { has_passed_conceptual: true, has_passed_practice: true }
    }

    fn closure_block() -> GeneratedSectionBlock {
        GeneratedSectionBlock::MetacognitiveClosure {
            synthesis_task: "t".to_string(),
            prediction_comparison: PredictionComparison {
                initial_prediction: "ip".to_string(),
                final_result: "fr".to_string(),
                contrast_narrative: "cn".to_string(),
            },
            self_evaluation_checklist: vec!["c".to_string()],
        }
    }

    #[test]
    fn flags_consecutive_repeated_block_type() {
        let v = single_block_violations(&[DynamicBlockType::InteractivePredictionGate], &valid_gate(), 2, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("se repite consecutivamente")), "{v:?}");
    }

    #[test]
    fn flags_correct_option_not_in_options() {
        let mut gate = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { correct_option, .. } = &mut gate {
            *correct_option = "Z".to_string();
        }
        let v = single_block_violations(&[], &gate, 1, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("correctOption")), "{v:?}");
    }

    #[test]
    fn flags_word_cap_on_anchored_micro_theory_other_layers() {
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "palabra ".repeat(101),
            analogy_boundary: None,
            system_rule: "r".to_string(),
            frequent_error: "e".to_string(),
        };
        let v = single_block_violations(&[], &block, 1, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("máximo 100 combinadas")), "{v:?}");
    }

    #[test]
    fn flags_word_cap_on_anchored_micro_theory_system_rule() {
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "h".to_string(),
            analogy_boundary: None,
            system_rule: "palabra ".repeat(241),
            frequent_error: "e".to_string(),
        };
        let v = single_block_violations(&[], &block, 1, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("systemRule tiene") && s.contains("máximo 240")), "{v:?}");
    }

    #[test]
    fn allows_system_rule_up_to_its_own_240_word_ceiling_even_with_a_near_full_other_layers_budget() {
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "palabra ".repeat(40).trim().to_string(),
            analogy_boundary: Some("palabra ".repeat(30).trim().to_string()),
            system_rule: "palabra ".repeat(240).trim().to_string(),
            frequent_error: "palabra ".repeat(29).trim().to_string(),
        };
        let v = single_block_violations(&[], &block, 1, false, no_mastery(), false, false);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn flags_metacognitive_closure_before_mastery_is_complete_unless_forced() {
        let block = closure_block();
        let too_early = single_block_violations(&[], &block, 4, false, no_mastery(), false, false);
        assert!(too_early.iter().any(|s| s.contains("antes de tiempo")), "{too_early:?}");

        let partial = single_block_violations(
            &[],
            &block,
            4,
            false,
            MasteryProgress { has_passed_conceptual: true, has_passed_practice: false },
            false,
            false,
        );
        assert!(partial.iter().any(|s| s.contains("compuerta de práctica")), "{partial:?}");

        let ok = single_block_violations(&[], &block, 6, false, full_mastery(), false, false);
        assert!(!ok.iter().any(|s| s.contains("antes de tiempo")), "{ok:?}");
    }

    #[test]
    fn force_close_bypasses_the_mastery_requirement_but_demands_a_closure_block() {
        let closure = closure_block();
        let v = single_block_violations(&[], &closure, 21, false, no_mastery(), true, false);
        assert!(!v.iter().any(|s| s.contains("antes de tiempo")), "{v:?}");

        let non_closure = valid_gate();
        let v = single_block_violations(&[], &non_closure, 21, false, no_mastery(), true, false);
        assert!(v.iter().any(|s| s.contains("cierre inmediato")), "{v:?}");
    }

    #[test]
    fn mastery_progress_only_counts_passed_gates_of_each_category() {
        fn block(block_type: DynamicBlockType, status: BlockStatus) -> NotebookBlock {
            NotebookBlock {
                id: "id".to_string(),
                document_id: "doc".to_string(),
                block_type,
                content_json: serde_json::json!({}),
                order_index: 0,
                status,
                attempt_count: 0,
                last_feedback: None,
            }
        }
        assert!(!mastery_progress(&[]).complete());
        let only_conceptual = [block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed)];
        assert!(!mastery_progress(&only_conceptual).complete());
        let failed_practice = [
            block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed),
            block(DynamicBlockType::HandsOnMission, BlockStatus::Failed),
        ];
        assert!(!mastery_progress(&failed_practice).complete(), "a FAILED practice gate must not count");
        let both = [
            block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed),
            block(DynamicBlockType::HandsOnMission, BlockStatus::Passed),
        ];
        assert!(mastery_progress(&both).complete());
    }

    #[test]
    fn skips_the_safety_ceiling_only_during_escalation() {
        let v_normal = single_block_violations(&[], &valid_gate(), MAX_TOTAL_BLOCKS + 1, false, no_mastery(), false, false);
        assert!(v_normal.iter().any(|s| s.contains("tope de seguridad")), "{v_normal:?}");
        let v_escalation = single_block_violations(&[], &valid_gate(), MAX_TOTAL_BLOCKS + 1, true, no_mastery(), false, false);
        assert!(!v_escalation.iter().any(|s| s.contains("tope de seguridad")), "{v_escalation:?}");
    }

    #[test]
    fn spaced_retrieval_is_required_as_the_first_block_when_due_and_forbidden_later() {
        let retrieval = GeneratedSectionBlock::SpacedInterleavedRetrieval {
            items: vec![crate::domain::notebook::RetrievalPrompt {
                concept_label: "Ósmosis".to_string(),
                prompt: "¿Hacia dónde se mueve el agua?".to_string(),
                expected_answer: "Hacia la mayor concentración de soluto".to_string(),
            }],
        };
        let missing = single_block_violations(&[], &valid_gate(), 1, false, no_mastery(), false, true);
        assert!(missing.iter().any(|s| s.contains("spaced_interleaved_retrieval")), "{missing:?}");

        let ok_first = single_block_violations(&[], &retrieval, 1, false, no_mastery(), false, true);
        assert!(!ok_first.iter().any(|s| s.contains("PRIMER bloque")), "{ok_first:?}");

        let too_late =
            single_block_violations(&[DynamicBlockType::AnchoredMicroTheory], &retrieval, 2, false, no_mastery(), false, false);
        assert!(too_late.iter().any(|s| s.contains("PRIMER bloque")), "{too_late:?}");
    }

    #[test]
    fn dominance_violation_only_fires_once_a_type_dominates_two_or_more_prior_classes() {
        let block = GeneratedSectionBlock::HeuristicErrorAudit {
            instruction: "i".to_string(),
            flawed_representation: crate::domain::notebook::FlawedRepresentation {
                context: "c".to_string(),
                buggy_snippet_or_diagram: "s".to_string(),
                error_type: crate::domain::notebook::FlawErrorType::Syntax,
            },
            guiding_questions: vec!["q".to_string()],
            model_solution: "sol".to_string(),
            faulty_line_number: None,
            visual_aid: None,
        };
        let usage: HashMap<String, i64> = [("heuristic_error_audit".to_string(), 2i64)].into_iter().collect();
        assert!(dominance_violation(&block, &(usage.clone(), 1)).is_none(), "only 1 prior class — not enough history yet");
        assert!(dominance_violation(&block, &(usage, 2)).is_some(), "2 of 2 prior classes already used it");
    }

    #[test]
    fn a_state_diagram_label_with_a_double_colon_is_rejected_but_other_chart_types_are_not() {
        fn gate_with(chart_type: MermaidChartType, code: &str) -> GeneratedSectionBlock {
            GeneratedSectionBlock::InteractivePredictionGate {
                question: "q".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
                conceptual_feedback_map: [("A".to_string(), "a".to_string()), ("B".to_string(), "b".to_string())]
                    .into_iter()
                    .collect(),
                correct_option: "B".to_string(),
                interaction_mode: None,
                shuffled_steps: None,
                correct_order: None,
                boundary_cases: None,
                visual_aid: Some(StaticVisualSpec::Mermaid {
                    chart_type,
                    code: code.to_string(),
                    caption: "c".to_string(),
                }),
            }
        }

        let broken = gate_with(
            MermaidChartType::StateDiagram,
            "stateDiagram-v2\n    [*] --> Valido: let nota = String::from(\"idea\")",
        );
        let v = single_block_violations(&[], &broken, 1, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("stateDiagram") && s.contains("::")), "{v:?}");

        let fine = gate_with(MermaidChartType::StateDiagram, "stateDiagram-v2\n    [*] --> Valido: el valor queda prestado");
        let v = single_block_violations(&[], &fine, 1, false, no_mastery(), false, false);
        assert!(!v.iter().any(|s| s.contains("stateDiagram")), "a natural-language state label is fine: {v:?}");

        let flowchart = gate_with(MermaidChartType::Flowchart, "flowchart LR\n    A[usa String::from] --> B[ok]");
        let v = single_block_violations(&[], &flowchart, 1, false, no_mastery(), false, false);
        assert!(!v.iter().any(|s| s.contains("::")), "`::` parses fine outside stateDiagram: {v:?}");
    }

    #[test]
    fn a_transfer_mission_may_follow_a_mission_and_may_have_no_hints_but_an_ordinary_repeat_is_still_rejected() {
        let mission = |transfer: bool| GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "c".to_string(),
            expected_milestone_artifact: "a".to_string(),
            constraints: vec!["k".to_string()],
            scaffolding_hints: if transfer { vec![] } else { vec!["h".to_string()] },
            evaluation_rubric_summary: vec!["r".to_string()],
            is_transfer: transfer,
            support_level: Some(if transfer { MissionSupportLevel::Independent } else { MissionSupportLevel::Faded }),
            worked_template: if transfer { None } else { Some("Completa: ___".to_string()) },
            interactive_steps: None,
            visual_aid: None,
        };
        let prior = [DynamicBlockType::HandsOnMission];
        let ok = single_block_violations(&prior, &mission(true), 2, false, full_mastery(), false, false);
        assert!(ok.is_empty(), "transfer after a mission is a new case, not a repeat: {ok:?}");
        let repeated = single_block_violations(&prior, &mission(false), 2, false, full_mastery(), false, false);
        assert!(repeated.iter().any(|m| m.contains("se repite consecutivamente")));

        let usage: HashMap<String, i64> = [("hands_on_mission".to_string(), 3i64)].into_iter().collect();
        assert!(dominance_violation(&mission(true), &(usage.clone(), 3)).is_none(), "transfer is exempt from the variety rule");
        assert!(dominance_violation(&mission(false), &(usage, 3)).is_some());
    }

    fn mission_with(level: Option<MissionSupportLevel>, transfer: bool, template: Option<&str>) -> GeneratedSectionBlock {
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "c".to_string(),
            expected_milestone_artifact: "a".to_string(),
            constraints: vec!["k".to_string()],
            scaffolding_hints: if transfer { vec![] } else { vec!["h".to_string()] },
            evaluation_rubric_summary: vec!["r".to_string()],
            is_transfer: transfer,
            support_level: level,
            worked_template: template.map(str::to_string),
            interactive_steps: None,
            visual_aid: None,
        }
    }

    const THEORY: DynamicBlockType = DynamicBlockType::AnchoredMicroTheory;

    #[test]
    fn theory_never_jumps_straight_to_an_independent_mission() {
        let independent = mission_with(Some(MissionSupportLevel::Independent), false, None);
        let v = single_block_violations(&[THEORY], &independent, 2, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("independent")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("primer bloque práctico")), "{v:?}");
    }

    #[test]
    fn the_first_practice_after_a_diagram_must_be_guided_or_faded_with_something_to_start_from() {
        let diagram = DynamicBlockType::DeclarativeVisualDiagram;
        let full = mission_with(Some(MissionSupportLevel::Full), false, Some("___"));
        assert!(single_block_violations(&[THEORY, diagram], &full, 3, false, no_mastery(), false, false).iter().any(|s| s.contains("primer bloque práctico")));

        let blank_page = mission_with(Some(MissionSupportLevel::Guided), false, None);
        let v = single_block_violations(&[THEORY], &blank_page, 2, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("workedTemplate")), "{v:?}");

        let ok = mission_with(Some(MissionSupportLevel::Faded), false, Some("let x = ___;"));
        assert!(single_block_violations(&[THEORY], &ok, 2, false, no_mastery(), false, false).is_empty());
    }

    #[test]
    fn a_checking_gate_between_theory_and_mission_lifts_the_first_practice_rule() {
        // theory -> gate (any outcome) -> mission: the bridge exists, so `full` is not flagged as a jump.
        let gate = DynamicBlockType::InteractivePredictionGate;
        let full = mission_with(Some(MissionSupportLevel::Full), false, Some("___"));
        let v = single_block_violations(&[THEORY, gate], &full, 3, false, no_mastery(), false, false);
        assert!(!v.iter().any(|s| s.contains("primer bloque práctico")), "{v:?}");
    }

    /// Golden progression violation text (user-facing, kept verbatim in sync
    /// with the guardrail in `single_block_violations`).
    const AUDIT_RIGHT_AFTER_FIRST_THEORY: &str = "Para un principiante, el bloque inmediatamente posterior a la primera \
         teoría debe practicar primero el caso básico correcto (interactive_prediction_gate sobre el caso normal o \
         práctica guiada paso a paso), nunca saltar directo a auditar errores o casos borde sin práctica previa";

    fn audit_block() -> GeneratedSectionBlock {
        GeneratedSectionBlock::HeuristicErrorAudit {
            instruction: "i".to_string(),
            flawed_representation: crate::domain::notebook::FlawedRepresentation {
                context: "c".to_string(),
                buggy_snippet_or_diagram: "s".to_string(),
                error_type: crate::domain::notebook::FlawErrorType::Syntax,
            },
            guiding_questions: vec!["q".to_string()],
            model_solution: "sol".to_string(),
            faulty_line_number: None,
            visual_aid: None,
        }
    }

    #[test]
    fn rejects_an_error_audit_immediately_after_the_first_theory_until_a_happy_path_gate_practices_the_basic_case() {
        let v = single_block_violations(&[THEORY], &audit_block(), 2, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s == AUDIT_RIGHT_AFTER_FIRST_THEORY), "{v:?}");

        let after_practice = [THEORY, DynamicBlockType::InteractivePredictionGate];
        let ok = single_block_violations(&after_practice, &audit_block(), 3, false, no_mastery(), false, false);
        assert!(!ok.iter().any(|s| s == AUDIT_RIGHT_AFTER_FIRST_THEORY), "{ok:?}");
    }

    #[test]
    fn the_audit_after_first_theory_rule_is_exempt_during_escalation_or_regeneration_and_ignores_position_zero() {
        // The fused `is_escalation || is_regeneration` flag repairs blocks IN
        // PLACE — without the exemption a pre-existing audit at this position
        // could never be regenerated.
        let escalated = single_block_violations(&[THEORY], &audit_block(), 2, true, no_mastery(), false, false);
        assert!(!escalated.iter().any(|s| s == AUDIT_RIGHT_AFTER_FIRST_THEORY), "{escalated:?}");

        // Position 0 (audit as the class opener) is outside this rule.
        let opener = single_block_violations(&[], &audit_block(), 1, false, no_mastery(), false, false);
        assert!(!opener.iter().any(|s| s == AUDIT_RIGHT_AFTER_FIRST_THEORY), "{opener:?}");
    }

    #[test]
    fn a_mission_must_declare_its_level_and_independent_is_reserved_for_transfer() {
        let undeclared = mission_with(None, false, None);
        assert!(single_block_violations(&[], &undeclared, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("declara supportLevel")));

        let transfer = mission_with(Some(MissionSupportLevel::Independent), true, None);
        assert!(single_block_violations(&[THEORY, DynamicBlockType::InteractivePredictionGate], &transfer, 3, false, full_mastery(), false, false).is_empty());

        let guided_transfer = mission_with(Some(MissionSupportLevel::Guided), true, None);
        assert!(single_block_violations(&[], &guided_transfer, 1, false, full_mastery(), false, false).iter().any(|s| s.contains("independent")));
    }

    #[test]
    fn an_escalation_may_answer_a_mission_with_a_more_scaffolded_mission_but_nothing_else_repeats() {
        let easier = mission_with(Some(MissionSupportLevel::Faded), false, Some("___"));
        let prior = [DynamicBlockType::HandsOnMission];
        assert!(single_block_violations(&prior, &easier, 2, true, no_mastery(), false, false).is_empty());
        // Same block outside an escalation is still a consecutive repeat.
        assert!(single_block_violations(&prior, &easier, 2, false, no_mastery(), false, false).iter().any(|s| s.contains("se repite consecutivamente")));
    }

    #[test]
    fn mission_interactive_steps_and_templates_are_validated() {
        let mut block = mission_with(Some(MissionSupportLevel::Faded), false, Some("sin huecos"));
        if let GeneratedSectionBlock::HandsOnMission { interactive_steps, .. } = &mut block {
            *interactive_steps = Some(vec![
                crate::domain::notebook::InteractiveStep { prompt: "p".into(), expected_token: "x".into(), options: Some(vec!["a".into(), "b".into()]) },
                crate::domain::notebook::InteractiveStep { prompt: "".into(), expected_token: "y".into(), options: None },
            ]);
        }
        let v = single_block_violations(&[THEORY], &block, 2, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("workedTemplate no tiene huecos")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("interactiveSteps[0].options")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("interactiveSteps[1]")), "{v:?}");
    }

    #[test]
    fn faulty_line_number_must_point_inside_the_snippet() {
        let audit = |line: Option<u32>| GeneratedSectionBlock::HeuristicErrorAudit {
            instruction: "i".to_string(),
            flawed_representation: crate::domain::notebook::FlawedRepresentation {
                context: "c".to_string(),
                buggy_snippet_or_diagram: "a = 1\nb = a +\nc = b".to_string(),
                error_type: crate::domain::notebook::FlawErrorType::Syntax,
            },
            guiding_questions: vec!["q".to_string()],
            model_solution: "m".to_string(),
            faulty_line_number: line,
            visual_aid: None,
        };
        assert!(single_block_violations(&[], &audit(Some(2)), 1, false, no_mastery(), false, false).is_empty());
        assert!(single_block_violations(&[], &audit(None), 1, false, no_mastery(), false, false).is_empty());
        for bad in [0, 4] {
            let v = single_block_violations(&[], &audit(Some(bad)), 1, false, no_mastery(), false, false);
            assert!(v.iter().any(|s| s.contains("faultyLineNumber")), "{bad}: {v:?}");
        }
    }

    fn gate_in_mode(mode: InteractionMode, shuffled: &[&str], order: &[&str], cases: Option<Vec<(&str, bool)>>) -> GeneratedSectionBlock {
        GeneratedSectionBlock::InteractivePredictionGate {
            question: "q".to_string(),
            options: vec![],
            conceptual_feedback_map: Default::default(),
            correct_option: String::new(),
            interaction_mode: Some(mode),
            shuffled_steps: Some(shuffled.iter().map(|s| s.to_string()).collect()),
            correct_order: Some(order.iter().map(|s| s.to_string()).collect()),
            boundary_cases: cases.map(|c| {
                c.into_iter().map(|(statement, follows_rule)| crate::domain::notebook::BoundaryCase { statement: statement.to_string(), follows_rule }).collect()
            }),
            visual_aid: None,
        }
    }

    #[test]
    fn causal_chain_gates_need_the_same_links_in_a_different_order() {
        let ok = gate_in_mode(InteractionMode::CausalChainOrder, &["B", "A", "C"], &["A", "B", "C"], None);
        assert!(single_block_violations(&[], &ok, 1, false, no_mastery(), false, false).is_empty());

        let already_ordered = gate_in_mode(InteractionMode::CausalChainOrder, &["A", "B", "C"], &["A", "B", "C"], None);
        assert!(single_block_violations(&[], &already_ordered, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("ya viene en el orden correcto")));

        let mismatched = gate_in_mode(InteractionMode::CausalChainOrder, &["B", "A", "C"], &["A", "B", "Z"], None);
        assert!(single_block_violations(&[], &mismatched, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("EXACTAMENTE los mismos eslabones")));

        let too_short = gate_in_mode(InteractionMode::CausalChainOrder, &["B", "A"], &["A", "B"], None);
        assert!(single_block_violations(&[], &too_short, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("3-4")));
    }

    #[test]
    fn boundary_case_gates_need_a_real_contrast() {
        let ok = gate_in_mode(InteractionMode::BoundaryCaseAudit, &[], &[], Some(vec![("c1", true), ("c2", false)]));
        assert!(single_block_violations(&[], &ok, 1, false, no_mastery(), false, false).is_empty());

        let one_sided = gate_in_mode(InteractionMode::BoundaryCaseAudit, &[], &[], Some(vec![("c1", true), ("c2", true)]));
        assert!(single_block_violations(&[], &one_sided, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("contraste real")));

        let single = gate_in_mode(InteractionMode::BoundaryCaseAudit, &[], &[], Some(vec![("c1", true)]));
        assert!(single_block_violations(&[], &single, 1, false, no_mastery(), false, false).iter().any(|s| s.contains("2-3")));
    }

    #[test]
    fn every_expanded_template_passes_the_structural_review() {
        use crate::domain::block_templates::{BlockTemplate, BranchSpec, OptionSpec};
        use crate::domain::notebook::RetrievalPrompt;
        let opt = |t: &str| OptionSpec { text: t.to_string(), feedback: format!("porque {t}") };
        let br = |c: &str| BranchSpec { choice: c.to_string(), consequence: format!("{c} lleva a algo") };
        let templates = vec![
            BlockTemplate::MultipleChoice { question: "q".into(), correct: opt("A"), distractors: vec![opt("B"), opt("C")] },
            BlockTemplate::TrueFalse { statement: "s".into(), is_true: true, explanation: "e".into() },
            BlockTemplate::Ordering { question: "q".into(), steps_in_order: vec!["uno".into(), "dos".into(), "tres".into()] },
            BlockTemplate::ScenarioChoice { scenario: "s".into(), decision_point: "d".into(), optimal: br("x"), alternatives: vec![br("y")] },
        ];
        for t in templates {
            let block = t.expand();
            let v = single_block_violations(&[DynamicBlockType::AnchoredMicroTheory], &block, 2, false, no_mastery(), false, false);
            assert!(v.is_empty(), "{:?} -> {v:?}", block.block_type());
        }
        let recall = BlockTemplate::QuickRecall { items: vec![RetrievalPrompt { concept_label: "c".into(), prompt: "p".into(), expected_answer: "e".into() }] }.expand();
        // Retrieval is only legal as the first block, so it is checked as such.
        let v = single_block_violations(&[], &recall, 1, false, no_mastery(), false, true);
        assert!(v.is_empty(), "{v:?}");
    }
}
