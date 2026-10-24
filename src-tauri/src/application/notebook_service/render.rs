//! Turns application state into the plain-text/JSON input `notebook_agent`
//! and `notebook_gate_grader` actually receive. Mirrors `roadmap_service::
//! render`'s role: keep the string-building out of the orchestration logic
//! in `generation`/`grading`.

use std::collections::HashMap;

use crate::domain::learner_memory::LearnerCognitiveMemory;
use crate::domain::lesson_composition::{infer_profile, suggest_next_block, suggested_sequence};
use crate::domain::resources::ClassResource;
use crate::domain::notebook::{BlockStatus, DynamicBlockType, GateAttempt, GeneratedSectionBlock, NotebookBlock};
use crate::domain::scaffolding;
use crate::notebook_store::ClassGenerationContext;
use crate::prompt_encoding::toon_payload;

use super::grounding::MasteryProgress;

/// What each prior block of this class actually TAUGHT, not just which slot
/// it filled: `{blockType, content}` per block, in order, with `visualAid`
/// stripped (its SVG geometry is the bulk of a block's bytes and carries no
/// prose the model could accidentally repeat). Both the generator and the
/// critic need this — a bare list of type names says nothing about WHAT was
/// explained, so the generator would re-explain the same rule in new words
/// and the critic could not tell. A class caps at
/// `grounding::MAX_TOTAL_BLOCKS` (20) and each block's text is word-capped,
/// so this stays small.
fn prior_blocks_content(existing_blocks: &[NotebookBlock]) -> Vec<serde_json::Value> {
    existing_blocks
        .iter()
        .map(|b| {
            let mut content = b.content_json.clone();
            if let Some(obj) = content.as_object_mut() {
                obj.remove("visualAid");
                // The learner's private notes/selection never feed the model.
                obj.remove("studentNote");
                obj.remove("studentState");
            }
            serde_json::json!({ "blockType": b.block_type.as_str(), "content": content })
        })
        .collect()
}

/// The curated web resources of this class as the generator should read them:
/// title, kind and what to do first — DELIBERATELY without the URL. The app
/// shows the links in its own panel, so the generator names a resource by its
/// title and has no link it could mistype or invent in a block.
fn class_resources_json(resources: &[ClassResource]) -> serde_json::Value {
    serde_json::Value::Array(
        resources
            .iter()
            .map(|r| serde_json::json!({ "title": r.title, "kind": r.kind, "why": r.why, "whatToDo": r.what_to_do }))
            .collect(),
    )
}

/// Reduces `LearnerCognitiveMemory` down to exactly what ONE block-generation
/// call needs to see — never the raw store row (timestamps, EMA internals):
/// due retrieval items already resolved to a boolean-shaped list, relevant
/// misconceptions already matched against this class's own title/goal, and
/// the two personalization directives as plain strings the prompt already
/// knows how to read. `None` fields are simply omitted from the JSON so the
/// model isn't asked to reason about an empty personalization with nothing
/// to say.
fn render_learner_memory_context(memory: &LearnerCognitiveMemory, ctx: &ClassGenerationContext, now_ms: i64) -> serde_json::Value {
    let due: Vec<serde_json::Value> = memory
        .due_retrieval_items(now_ms)
        .iter()
        .map(|i| serde_json::json!({ "conceptId": i.concept_id, "conceptLabel": i.concept_label }))
        .collect();
    let relevance_text = format!("{} {} {}", ctx.course.title, ctx.course.target_goal, ctx.class.title);
    let misconceptions: Vec<serde_json::Value> = memory
        .misconceptions_relevant_to(&relevance_text)
        .iter()
        .map(|m| serde_json::json!({ "domainConcept": m.domain_concept, "identifiedErrorPattern": m.identified_error_pattern }))
        .collect();
    let mut out = serde_json::json!({
        "calibratedBaseline": memory.calibrated_baseline,
        "dueRetrieval": due,
        "relevantMisconceptions": misconceptions,
    });
    if memory.needs_heavy_scaffolding() {
        out["scaffoldingDirective"] = serde_json::Value::String(
            "andamiaje_fuerte: el estudiante parte de cero o su precisión reciente en predicciones es baja — antes \
             de cualquier compuerta autónoma, dedica un anchored_micro_theory (o declarative_visual_diagram) extra, \
             muy guiado, con la analogía más concreta posible."
                .to_string(),
        );
    }
    if memory.friction_is_low() {
        out["frictionDirective"] = serde_json::Value::String(
            "tolerancia_baja_a_la_friccion: este estudiante se frustra rápido — el sistema ya reduce el umbral de \
             escalación a 1 solo fallo (en vez de 3). Cuando generes un bloque de re-enfoque para este estudiante, \
             prioriza SIEMPRE el ángulo más visual/concreto disponible (declarative_visual_diagram > \
             anchored_micro_theory con analogía) antes que otro acertijo abstracto."
                .to_string(),
        );
    }
    out
}

/// The student's real answers on this class's gates, oldest first — what the
/// closing block must quote instead of inventing a "prediction". `blockNumber`
/// is the 1-based position the student saw; answers are capped so a long
/// free-text mission can't bloat every generation call.
fn gate_history_json(existing_blocks: &[NotebookBlock], attempts: &[GateAttempt]) -> serde_json::Value {
    const MAX_ANSWER_CHARS: usize = 400;
    let rows: Vec<serde_json::Value> = attempts
        .iter()
        .map(|a| {
            let block = existing_blocks.iter().find(|b| b.id == a.block_id);
            let prompt = block.and_then(|b| {
                ["question", "challengeStatement", "scenario", "instruction"]
                    .iter()
                    .find_map(|k| b.content_json.get(*k).and_then(|v| v.as_str()))
                    .map(|s| s.chars().take(160).collect::<String>())
            });
            serde_json::json!({
                "blockNumber": block.map(|b| b.order_index + 1),
                "blockType": a.block_type.as_str(),
                "gatePrompt": prompt,
                "attempt": a.attempt_number,
                "studentAnswer": a.answer_text.chars().take(MAX_ANSWER_CHARS).collect::<String>(),
                "passed": a.passed,
            })
        })
        .collect();
    let first_failed = attempts.first().is_some_and(|a| !a.passed);
    serde_json::json!({
        "attempts": rows,
        "firstAttemptWasWrong": first_failed,
        "note": "Respuestas REALES del estudiante, en orden. Cítalas textualmente (predictionComparison.initialPrediction = la primera; \
                 contrastNarrative debe mencionar en qué intento acertó y qué cambió); nunca inventes una predicción.",
    })
}

/// The `compositionHint` for the next block: the class's inferred
/// [`DisciplineProfile`](crate::domain::lesson_composition::DisciplineProfile),
/// its suggested chain, and the block that belongs at the next position —
/// minus anything that already dominates the course's earlier classes. Only a
/// starting disposition: the generator may deviate (and must say why in
/// `pedagogicalRationale`). `None` when the class text matches no profile.
fn composition_hint(
    ctx: &ClassGenerationContext,
    existing_blocks: &[NotebookBlock],
    mastery: MasteryProgress,
    (usage, total_classes): &(HashMap<String, i64>, i64),
) -> Option<serde_json::Value> {
    let profile = infer_profile(&ctx.class.title, &[&ctx.milestone.title, &ctx.course.title, &ctx.course.target_goal])?;
    let prior: Vec<DynamicBlockType> = existing_blocks.iter().map(|b| b.block_type).collect();
    // Practice-family types that already dominate this course (same >50% rule
    // as `grounding::dominance_violation`, which would reject them anyway).
    let dominant: Vec<DynamicBlockType> = if *total_classes >= 2 {
        usage
            .iter()
            .filter(|(_, n)| (**n as f64) / (*total_classes as f64) > 0.5)
            .filter_map(|(k, _)| DynamicBlockType::parse(k))
            .filter(|t| {
                matches!(t, DynamicBlockType::HeuristicErrorAudit | DynamicBlockType::HandsOnMission | DynamicBlockType::BranchingScenarioChallenge)
            })
            .collect()
    } else {
        Vec::new()
    };
    let suggested = suggest_next_block(profile, &prior, mastery.complete(), &dominant);
    Some(serde_json::json!({
        "disciplineProfile": profile.as_str(),
        "profileSequence": suggested_sequence(profile).iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        "nextPosition": prior.iter().filter(|t| **t != DynamicBlockType::SpacedInterleavedRetrieval).count() + 1,
        "suggestedBlockType": suggested.map(|t| t.as_str()),
        "avoidBlockTypes": dominant.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        "guidance": "Perfil inferido por palabras clave del tema (corrígelo en pedagogicalRationale si el tema de HOY es de otro perfil). \
                     suggestedBlockType es el punto de partida para ESTA posición, no un molde: respétalo salvo que scaffolding, \
                     masteryStatus o contentSoFar pidan otra cosa. No uses el mismo tipo de práctica que en las clases anteriores \
                     (avoidBlockTypes) ni dos veces seguidas.",
    }))
}

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
    learner_memory: &LearnerCognitiveMemory,
    now_ms: i64,
    gate_attempts: &[GateAttempt],
    resources: &[ClassResource],
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
            "deliverable": ctx.milestone.deliverable,
        },
        "class": {
            "title": ctx.class.title,
            "classNumber": ctx.class.class_number,
            "allocatedHours": ctx.class.hours,
        },
        "blocksSoFar": existing_blocks.iter().map(|b| b.block_type.as_str()).collect::<Vec<_>>(),
        "contentSoFar": prior_blocks_content(existing_blocks),
        "initialPrediction": initial_prediction.unwrap_or("(el estudiante aún no ha registrado una predicción inicial en esta clase)"),
        "finalResultSoFar": final_result_so_far,
        "masteryStatus": {
            "hasPassedConceptualGate": mastery.has_passed_conceptual,
            "hasPassedPracticeGate": mastery.has_passed_practice,
            "canClose": mastery.has_passed_conceptual && mastery.has_passed_practice,
        },
    });
    // The deterministic review rejects two adjacent blocks of one type, and a
    // rejection costs a whole extra generation — say it up front.
    if let Some(last) = existing_blocks.last() {
        context["cannotRepeatBlockType"] = serde_json::json!(last.block_type.as_str());
    }
    if let Some(profile) = diagnostic_profile {
        context["diagnosticProfile"] = profile.clone();
    }
    context["learnerMemory"] = render_learner_memory_context(learner_memory, ctx, now_ms);
    // "Extra theory first" can't be honored right after a theory/diagram block
    // (adjacent repeats are rejected), and it fights `nextShouldCheck`. Once
    // the scaffolding theory is down, the directive becomes "check, guided".
    if matches!(
        existing_blocks.last().map(|b| b.block_type),
        Some(DynamicBlockType::AnchoredMicroTheory | DynamicBlockType::DeclarativeVisualDiagram)
    ) && context["learnerMemory"].get("scaffoldingDirective").is_some()
    {
        context["learnerMemory"]["scaffoldingDirective"] = serde_json::Value::String(
            "andamiaje_fuerte: la teoría guiada de apoyo ya está; el SIGUIENTE bloque es una compuerta chica o una práctica              guiada/faded que la compruebe, no más teoría."
                .to_string(),
        );
    }
    if !resources.is_empty() {
        context["resources"] = class_resources_json(resources);
    }
    if let Some(hint) = composition_hint(ctx, existing_blocks, mastery, block_type_usage) {
        context["compositionHint"] = hint;
    }
    if !gate_attempts.is_empty() {
        context["gateHistory"] = gate_history_json(existing_blocks, gate_attempts);
    }
    context["scaffolding"] = scaffolding::plan(existing_blocks, learner_memory.needs_heavy_scaffolding()).to_json(existing_blocks);
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
        toon_payload(&context)
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
    gate_attempts: &[GateAttempt],
) -> String {
    let mut context = serde_json::json!({
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
    if !gate_attempts.is_empty() {
        context["gateHistory"] = gate_history_json(existing_blocks, gate_attempts);
    }
    format!(
        "Se alcanzó el tope de seguridad de bloques de esta clase. Debes CERRAR AHORA MISMO llamando \
         a publish_notebook_block con metacognitive_closure — sin excepción, aunque el estudiante no \
         haya completado ambas dimensiones de maestría todavía (revísalas en masteryStatus). Sé \
         honesto en contrastNarrative sobre lo que sí se logró y lo que quedó pendiente, en vez de \
         fingir que la clase se dominó por completo.\n\n{}",
        toon_payload(&context)
    )
}

/// Input for the escalation ("re-approach") call after 3 failed attempts on
/// the same gate. `recent_wrong_submission` is the student's most recent
/// (3rd) failing answer — the only one actually retained (see
/// `domain::notebook::NotebookBlock`, which tracks `attempt_count` but not a
/// full submission history), so the input is honest about that instead of
/// claiming to carry all 3.
pub(super) fn render_escalation_block_input(
    ctx: &ClassGenerationContext,
    failed_block: &NotebookBlock,
    recent_wrong_submission: &str,
    voluntary: bool,
) -> String {
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
    let intro = if voluntary {
        "El estudiante PIDIÓ un paso previo más guiado en esta compuerta (sin haber fallado 3 veces). Genera el \
         bloque de apoyo según la sección MODO ESCALACIÓN → «paso previo pedido por el estudiante» de tu system \
         prompt: la MISMA habilidad con MÁS andamiaje (una hands_on_mission con supportLevel \"faded\", \
         workedTemplate e interactiveSteps cuando el bloque original era una misión), nunca revelando la solución \
         del bloque original. Llama a publish_notebook_block."
    } else {
        "El estudiante falló la misma compuerta 3 veces. Genera el bloque de re-enfoque según la \
         sección MODO ESCALACIÓN de tu system prompt, llamando a publish_notebook_block."
    };
    format!("{intro}\n\n{}", toon_payload(&context))
}

/// Input for `notebook_gate_grader`'s `grade_gate_submission` call — one
/// free-text submission against its block's reference solution/rubric.
pub(super) fn render_grading_input(block: &NotebookBlock, submission_text: &str, attempt_number: u32, selected_line: Option<u32>) -> String {
    let mut content = block.content_json.clone();
    if let Some(obj) = content.as_object_mut() {
        obj.remove("studentNote");
        obj.remove("studentState");
    }
    let mut context = serde_json::json!({
        "blockType": block.block_type.as_str(),
        "blockContent": content,
        "studentSubmission": submission_text,
        "attemptNumber": attempt_number,
    });
    // `heuristic_error_audit` "Bug Bounty": the line the student pointed at
    // before writing the diagnosis (compared by the grader against
    // `blockContent.faultyLineNumber`, when the block has one).
    if let Some(line) = selected_line {
        context["studentSelectedLine"] = serde_json::json!(line);
    }
    format!(
        "Califica esta entrega llamando a grade_gate_submission según tu system prompt.\n\n{}",
        toon_payload(&context)
    )
}

/// Input for the gate grader's `spaced_interleaved_retrieval` mode: one
/// prompt, its reference answer and what the student wrote.
pub(super) fn render_retrieval_grading_input(item: &serde_json::Value, answer: &str) -> String {
    let context = serde_json::json!({
        "blockType": "spaced_interleaved_retrieval",
        "blockContent": {
            "conceptLabel": item.get("conceptLabel"),
            "prompt": item.get("prompt"),
            "expectedAnswer": item.get("expectedAnswer"),
        },
        "studentSubmission": answer,
        "attemptNumber": 1,
    });
    format!(
        "Califica esta respuesta de repaso llamando a grade_gate_submission según tu system prompt \
         (modo spaced_interleaved_retrieval).\n\n{}",
        toon_payload(&context)
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
        toon_payload(&context)
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
        "blocksSoFar": prior_blocks_content(existing_blocks),
        "scaffolding": scaffolding::plan(existing_blocks, false).to_json(existing_blocks),
        "block": block,
    });
    format!(
        "Revisa este bloque llamando a submit_block_audit según tu system prompt.\n\n{}",
        toon_payload(&context)
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
        .map(|b| match b.attempt_count {
            0 | 1 => format!("{} (superada al primer intento)", b.block_type.as_str()),
            n => format!("{} (superada en el intento {n})", b.block_type.as_str()),
        })
        .collect();
    if passed.is_empty() {
        "El estudiante todavía no ha superado ninguna compuerta en esta clase.".to_string()
    } else {
        format!("Compuertas superadas hasta ahora en esta clase: {}.", passed.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::DynamicBlockType;

    fn block(order_index: u32, block_type: DynamicBlockType, content_json: serde_json::Value) -> NotebookBlock {
        NotebookBlock {
            id: format!("b{order_index}"),
            document_id: "doc".to_string(),
            block_type,
            content_json,
            order_index,
            status: BlockStatus::Ready,
            attempt_count: 0,
            last_feedback: None,
        }
    }

    #[test]
    fn prior_blocks_content_keeps_what_was_taught_and_drops_visual_geometry() {
        let blocks = [
            block(
                0,
                DynamicBlockType::AnchoredMicroTheory,
                serde_json::json!({ "title": "Pila", "systemRule": "LIFO: el último en entrar sale primero." }),
            ),
            block(
                1,
                DynamicBlockType::DeclarativeVisualDiagram,
                serde_json::json!({ "title": "Mapa de la pila", "visualAid": { "svg": "<svg/>" } }),
            ),
        ];

        let digest = prior_blocks_content(&blocks);

        assert_eq!(digest.len(), 2);
        assert_eq!(digest[0]["blockType"], "anchored_micro_theory");
        assert_eq!(digest[0]["content"]["systemRule"], "LIFO: el último en entrar sale primero.");
        assert_eq!(digest[1]["content"]["title"], "Mapa de la pila");
        assert!(digest[1]["content"].get("visualAid").is_none());
    }

    #[test]
    fn gate_history_quotes_the_real_answers_in_order_and_flags_a_wrong_first_attempt() {
        let gate = block(2, DynamicBlockType::InteractivePredictionGate, serde_json::json!({ "question": "¿Qué pasa primero?" }));
        let attempts = [
            GateAttempt { document_id: "doc".into(), block_id: gate.id.clone(), block_type: gate.block_type, attempt_number: 1, answer_text: "A".into(), passed: false, at_ms: 1 },
            GateAttempt { document_id: "doc".into(), block_id: gate.id.clone(), block_type: gate.block_type, attempt_number: 2, answer_text: "B".into(), passed: true, at_ms: 2 },
        ];
        let history = gate_history_json(&[gate], &attempts);
        assert_eq!(history["firstAttemptWasWrong"], true);
        assert_eq!(history["attempts"][0]["studentAnswer"], "A");
        assert_eq!(history["attempts"][0]["blockNumber"], 3);
        assert_eq!(history["attempts"][0]["gatePrompt"], "¿Qué pasa primero?");
        assert_eq!(history["attempts"][1]["passed"], true);
    }

    #[test]
    fn final_result_says_which_attempt_passed() {
        let mut first_try = block(0, DynamicBlockType::InteractivePredictionGate, serde_json::json!({}));
        first_try.status = BlockStatus::Passed;
        first_try.attempt_count = 1;
        let mut third_try = block(1, DynamicBlockType::HandsOnMission, serde_json::json!({}));
        third_try.status = BlockStatus::Passed;
        third_try.attempt_count = 3;
        let text = synthesize_final_result_so_far(&[first_try, third_try]);
        assert!(text.contains("interactive_prediction_gate (superada al primer intento)"), "{text}");
        assert!(text.contains("hands_on_mission (superada en el intento 3)"), "{text}");
    }
}
