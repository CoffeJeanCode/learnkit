//! Turns application state into the plain-text/JSON input `notebook_agent`
//! and `notebook_gate_grader` actually receive. Mirrors `roadmap_service::
//! render`'s role: keep the string-building out of the orchestration logic
//! in `generation`/`grading`.

use std::collections::HashMap;

use crate::domain::notebook::{BlockStatus, GeneratedSectionBlock, NotebookBlock};
use crate::notebook_store::ClassGenerationContext;

use super::grounding::MasteryProgress;

/// Input for a normal (non-escalation) `publish_notebook_block` call — used
/// for both the very first block (`existing_blocks` empty) and every
/// subsequent one. `initial_prediction`/`final_result_so_far` are always
/// included (not just when the model happens to pick
/// `metacognitive_closure`) since the render layer can't predict which
/// block type the model will choose next — the prompt itself instructs the
/// model to only use them when it does close the notebook. `mastery` mirrors
/// the server-side closure-eligibility check (`grounding::
/// single_block_violations`) so the model can see for itself whether closing
/// is even allowed yet, instead of finding out only via a rejection.
pub(super) fn render_next_block_input(
    ctx: &ClassGenerationContext,
    existing_blocks: &[NotebookBlock],
    diagnostic_profile: Option<&serde_json::Value>,
    block_type_usage: &(HashMap<String, i64>, i64),
    initial_prediction: Option<&str>,
    final_result_so_far: &str,
    mastery: MasteryProgress,
) -> String {
    let mut context = serde_json::json!({
        "course": {
            "title": ctx.course.title,
            "targetGoal": ctx.course.target_goal,
            "totalWeeks": ctx.course.total_weeks,
        },
        "milestone": {
            "weekNumber": ctx.milestone.week_number,
            "title": ctx.milestone.title,
            "deliverableGoal": ctx.milestone.deliverable_goal,
        },
        "class": {
            "title": ctx.class.title,
            "classNumber": ctx.class.class_number,
            "allocatedHours": ctx.class.hours,
        },
        "blocksSoFar": existing_blocks.iter().map(|b| b.block_type.as_str()).collect::<Vec<_>>(),
        "initialPrediction": initial_prediction.unwrap_or("(el estudiante aún no ha registrado una predicción inicial en esta clase)"),
        "finalResultSoFar": final_result_so_far,
        "masteryStatus": {
            "hasPassedConceptualGate": mastery.has_passed_conceptual,
            "hasPassedPracticeGate": mastery.has_passed_practice,
            "canClose": mastery.has_passed_conceptual && mastery.has_passed_practice,
        },
    });
    if let Some(profile) = diagnostic_profile {
        context["diagnosticProfile"] = profile.clone();
    }
    if let Some(last) = existing_blocks.last() {
        if last.block_type.is_gate() && last.status == BlockStatus::Passed {
            context["lastGateOutcome"] = serde_json::json!({
                "blockType": last.block_type.as_str(),
                "outcome": "passed",
                "attempts": last.attempt_count,
            });
        }
    }
    let (usage, total_classes) = block_type_usage;
    if *total_classes > 0 {
        context["previousBlockTypeUsage"] = serde_json::json!({
            "totalClassesSoFar": total_classes,
            "classesUsingEachType": usage,
        });
    }
    format!(
        "Genera el SIGUIENTE bloque de esta clase, llamando a publish_notebook_block según tu system \
         prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Input for a FORCED closure call — the safety-ceiling escape hatch (see
/// `generation::generate_and_persist_block`'s `force_close` branch): the
/// notebook has hit `grounding::MAX_TOTAL_BLOCKS` without the student
/// completing both mastery dimensions. The model is compelled to close NOW,
/// honestly, rather than kept generating forever.
pub(super) fn render_forced_closure_input(
    ctx: &ClassGenerationContext,
    existing_blocks: &[NotebookBlock],
    initial_prediction: Option<&str>,
    final_result_so_far: &str,
    mastery: MasteryProgress,
) -> String {
    let context = serde_json::json!({
        "course": { "title": ctx.course.title, "targetGoal": ctx.course.target_goal },
        "class": { "title": ctx.class.title },
        "blocksSoFar": existing_blocks.iter().map(|b| b.block_type.as_str()).collect::<Vec<_>>(),
        "initialPrediction": initial_prediction.unwrap_or("(el estudiante aún no ha registrado una predicción inicial en esta clase)"),
        "finalResultSoFar": final_result_so_far,
        "masteryStatus": {
            "hasPassedConceptualGate": mastery.has_passed_conceptual,
            "hasPassedPracticeGate": mastery.has_passed_practice,
        },
    });
    format!(
        "Se alcanzó el tope de seguridad de bloques de esta clase. Debes CERRAR AHORA MISMO llamando \
         a publish_notebook_block con metacognitive_closure — sin excepción, aunque el estudiante no \
         haya completado ambas dimensiones de maestría todavía (revísalas en masteryStatus). Sé \
         honesto en contrastNarrative sobre lo que sí se logró y lo que quedó pendiente, en vez de \
         fingir que la clase se dominó por completo.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Input for the escalation ("re-approach") call after 3 failed attempts on
/// the same gate. `recent_wrong_submission` is the student's most recent
/// (3rd) failing answer — the only one actually retained (see
/// `domain::notebook::NotebookBlock`, which tracks `attempt_count` but not a
/// full submission history), so the input is honest about that instead of
/// claiming to carry all 3.
pub(super) fn render_escalation_block_input(ctx: &ClassGenerationContext, failed_block: &NotebookBlock, recent_wrong_submission: &str) -> String {
    let context = serde_json::json!({
        "course": {
            "title": ctx.course.title,
            "targetGoal": ctx.course.target_goal,
        },
        "class": { "title": ctx.class.title },
        "escalation": {
            "failedBlockType": failed_block.block_type.as_str(),
            "failedBlockContent": failed_block.content_json,
            "attemptCount": failed_block.attempt_count,
            "mostRecentWrongSubmission": recent_wrong_submission,
        },
    });
    format!(
        "El estudiante falló la misma compuerta 3 veces. Genera el bloque de re-enfoque según la \
         sección MODO ESCALACIÓN de tu system prompt, llamando a publish_notebook_block.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Input for `notebook_gate_grader`'s `grade_gate_submission` call — one
/// free-text submission against its block's reference solution/rubric.
pub(super) fn render_grading_input(block: &NotebookBlock, submission_text: &str, attempt_number: u32) -> String {
    let context = serde_json::json!({
        "blockType": block.block_type.as_str(),
        "blockContent": block.content_json,
        "studentSubmission": submission_text,
        "attemptNumber": attempt_number,
    });
    format!(
        "Califica esta entrega llamando a grade_gate_submission según tu system prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Input for `closure_feedback_grader`'s `grade_closure_submission` call —
/// the closing block's student reflection against its synthesis task and
/// self-evaluation checklist. The caller persists `reflection` into the
/// block's `studentReflection` BEFORE this runs, so `blockContent` and the
/// explicit field always carry the same text.
pub(super) fn render_closure_grading_input(block: &NotebookBlock, reflection: &str, attempt_number: u32) -> String {
    let context = serde_json::json!({
        "blockType": block.block_type.as_str(),
        "blockContent": block.content_json,
        "studentReflection": reflection,
        "attemptNumber": attempt_number,
    });
    format!(
        "Califica esta reflexión de cierre llamando a grade_closure_submission según tu system prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// Input for `pedagogical_critic`'s `submit_block_audit` call — the
/// semantic review of ONE freshly generated block, running only after the
/// deterministic guardrails already passed (see
/// `domain::pedagogy_guardrails::needs_llm_critic`).
pub(super) fn render_block_audit_input(
    ctx: &ClassGenerationContext,
    existing_blocks: &[NotebookBlock],
    block: &GeneratedSectionBlock,
) -> String {
    let context = serde_json::json!({
        "topic": ctx.class.title,
        "targetGoal": ctx.course.target_goal,
        "blockType": block.block_type().as_str(),
        "blocksSoFar": existing_blocks.iter().map(|b| b.block_type.as_str()).collect::<Vec<_>>(),
        "block": block,
    });
    format!(
        "Revisa este bloque llamando a submit_block_audit según tu system prompt.\n\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default()
    )
}

/// A short, truthful summary of gate blocks the student has already passed
/// in this notebook — fed to every block-generation call as
/// `finalResultSoFar` so a `metacognitive_closure` block (whenever the model
/// decides to generate one) has real material for its `predictionComparison
/// .finalResult`, instead of the model having to invent one from nothing.
pub(super) fn synthesize_final_result_so_far(existing_blocks: &[NotebookBlock]) -> String {
    let passed: Vec<String> = existing_blocks
        .iter()
        .filter(|b| b.block_type.is_gate() && b.status == BlockStatus::Passed)
        .map(|b| format!("{} (superada)", b.block_type.as_str()))
        .collect();
    if passed.is_empty() {
        "El estudiante todavía no ha superado ninguna compuerta en esta clase.".to_string()
    } else {
        format!("Compuertas superadas hasta ahora en esta clase: {}.", passed.join(", "))
    }
}
