//! Content-level grounding for ONE freshly generated block — the incremental
//! successor to the retired whole-notebook `notebook_grounding_violations`
//! (which judged an entire `GeneratedDynamicNotebook` at once). Now that
//! `notebook_agent` is called once per block (see `generation`), the same
//! rules — no two adjacent blocks of the same type, 3-5 total, per-block-type
//! field completeness, course-wide practice-family dominance — are checked
//! at EVERY block call instead of once at the end.

use std::collections::HashMap;

use crate::domain::notebook::{
    BlockStatus, DynamicBlockType, GeneratedSectionBlock, MermaidChartType, NotebookBlock, StaticVisualSpec,
};

/// The interchangeable "practice/activity" family — these three all serve
/// the same pedagogical ROLE (give the student something to DO with the
/// material) and are the ones a real model was observed defaulting to the
/// same member of, regardless of topic. Deliberately excludes
/// `anchored_micro_theory`/`declarative_visual_diagram`/
/// `interactive_prediction_gate`/`metacognitive_closure`: those serve fixed
/// STRUCTURAL roles that are SUPPOSED to recur across most classes.
const PRACTICE_BLOCK_FAMILY: [&str; 3] = ["heuristic_error_audit", "hands_on_mission", "branching_scenario_challenge"];

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
    if !PRACTICE_BLOCK_FAMILY.contains(&bt) {
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
    fn complete(self) -> bool {
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
    if let Some(last) = prior_types.last() {
        if *last == block.block_type() {
            v.push(format!(
                "blockType {} se repite consecutivamente respecto al bloque anterior — la secuencia debe variar",
                block.block_type().as_str()
            ));
        }
    }
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
        GeneratedSectionBlock::AnchoredMicroTheory { title, intuitive_hook, system_rule, frequent_error } => {
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
            let word_count = [intuitive_hook.as_str(), system_rule.as_str(), frequent_error.as_str()]
                .iter()
                .map(|s| s.split_whitespace().count())
                .sum::<usize>();
            if word_count > 160 {
                v.push(format!(
                    "bloque {i} (anchored_micro_theory): {word_count} palabras combinadas, máximo 160 — degrada en texto enciclopédico"
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
        GeneratedSectionBlock::HeuristicErrorAudit { instruction, flawed_representation, guiding_questions, model_solution, visual_aid } => {
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
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(i, visual));
            }
        }
        GeneratedSectionBlock::InteractivePredictionGate { question, options, conceptual_feedback_map, correct_option, visual_aid } => {
            if question.trim().is_empty() {
                v.push(empty("question"));
            }
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
            visual_aid,
        } => {
            if challenge_statement.trim().is_empty() {
                v.push(empty("challengeStatement"));
            }
            if expected_milestone_artifact.trim().is_empty() {
                v.push(empty("expectedMilestoneArtifact"));
            }
            if constraints.is_empty() {
                v.push(format!("bloque {i} (hands_on_mission): constraints está vacío"));
            }
            if scaffolding_hints.is_empty() {
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
    fn flags_word_cap_on_anchored_micro_theory() {
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "palabra ".repeat(161),
            system_rule: "r".to_string(),
            frequent_error: "e".to_string(),
        };
        let v = single_block_violations(&[], &block, 1, false, no_mastery(), false, false);
        assert!(v.iter().any(|s| s.contains("máximo 160")), "{v:?}");
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
}
