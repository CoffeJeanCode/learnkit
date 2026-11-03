//! Grades a `GateSubmission` against its block's stored answer key, and
//! answer-key redaction — the one place `content_json` is stripped of
//! `correctOption`/`isOptimal`/`modelSolution` before a block is allowed to
//! reach the frontend. Two block types (`interactive_prediction_gate`,
//! `branching_scenario_challenge`) grade deterministically in Rust, straight
//! against the stored content; the other two (`heuristic_error_audit`,
//! `hands_on_mission`) are free-text and go through `notebook_gate_grader`
//! (see `render::render_grading_input`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::AppHandle;

use crate::agents::closure_feedback_grader_agent::CLOSURE_FEEDBACK_GRADER_AGENT_ID;
use crate::agents::notebook_gate_grader_agent::NOTEBOOK_GATE_GRADER_AGENT_ID;
use crate::domain::skill_evidence::RubricCriterion;
use crate::domain::notebook::{BlockStatus, DynamicBlockType, NotebookBlock};
use crate::error::{AppError, AppResult};
use crate::orchestration::Orchestrator;
use crate::tools::{ClosureFeedbackCapture, ClosureFeedbackResult, GateGradingCapture, GateGradingResult};

use super::render;

const MAX_GRADING_ATTEMPTS: u8 = 2;
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(500);

/// `content_json` as persisted carries the answer key — the redacted copy
/// returned here is what every Tauri command must hand back to the
/// frontend. `status`-dependent because `heuristic_error_audit.
/// modelSolution` is meant to be revealed once the student actually passes
/// (see `HeuristicErrorAuditBlock.tsx`), unlike `correctOption`/`isOptimal`,
/// which are never sent to the client at all.
pub(super) fn redact_block(block: &NotebookBlock) -> NotebookBlock {
    let mut b = block.clone();
    if let Some(obj) = b.content_json.as_object_mut() {
        match b.block_type {
            DynamicBlockType::InteractivePredictionGate => {
                obj.remove("correctOption");
                obj.remove("correctOrder");
                if let Some(cases) = obj.get_mut("boundaryCases").and_then(|v| v.as_array_mut()) {
                    for case in cases.iter_mut().filter_map(|c| c.as_object_mut()) {
                        case.remove("followsRule");
                    }
                }
            }
            DynamicBlockType::BranchingScenarioChallenge => {
                if let Some(branches) = obj.get_mut("branches").and_then(|v| v.as_array_mut()) {
                    for br in branches {
                        if let Some(bo) = br.as_object_mut() {
                            bo.remove("isOptimal");
                        }
                    }
                }
            }
            DynamicBlockType::HeuristicErrorAudit => {
                if b.status != BlockStatus::Passed {
                    obj.remove("modelSolution");
                    obj.remove("faultyLineNumber");
                }
            }
            // Retention only counts if the student answered BEFORE seeing the
            // solution, so each item's `expectedAnswer` stays server-side
            // until that item has an outcome (`reportedOutcome`).
            DynamicBlockType::SpacedInterleavedRetrieval => {
                if let Some(items) = obj.get_mut("items").and_then(|v| v.as_array_mut()) {
                    for item in items.iter_mut().filter_map(|i| i.as_object_mut()) {
                        if !item.contains_key("reportedOutcome") {
                            item.remove("expectedAnswer");
                        }
                    }
                }
            }
            _ => {}
        }
    }
    b
}

pub(super) fn redact_all(blocks: Vec<NotebookBlock>) -> Vec<NotebookBlock> {
    blocks.iter().map(redact_block).collect()
}

/// What the student answered on an `interactive_prediction_gate`; which field
/// is set depends on the block's `interactionMode`.
#[derive(Default)]
pub(super) struct PredictionAnswer<'a> {
    pub selected_option: Option<&'a str>,
    pub ordered_steps: Option<&'a [String]>,
    pub case_verdicts: Option<&'a [bool]>,
}

/// Deterministic grade of a prediction gate in any of its modes. The second
/// element is a partial-credit hint for the two non-multiple-choice modes
/// (how many links/cases are right, never WHICH) — `None` on a pass and for
/// multiple choice, whose feedback comes from `conceptualFeedbackMap`.
pub(super) fn grade_prediction_gate(block: &NotebookBlock, answer: &PredictionAnswer) -> AppResult<(bool, Option<String>)> {
    let missing = |field: &str| AppError::Persistence(format!("interactive_prediction_gate block is missing {field}"));
    match block.content_json.get("interactionMode").and_then(|v| v.as_str()) {
        Some("causal_chain_order") => {
            let key: Vec<&str> = block
                .content_json
                .get("correctOrder")
                .and_then(|v| v.as_array())
                .ok_or_else(|| missing("correctOrder"))?
                .iter()
                .filter_map(|s| s.as_str())
                .collect();
            let Some(given) = answer.ordered_steps else { return Ok((false, None)) };
            if given.len() != key.len() {
                return Ok((false, None));
            }
            let in_place = given.iter().zip(&key).filter(|(g, k)| g.as_str() == **k).count();
            if in_place == key.len() {
                return Ok((true, None));
            }
            Ok((
                false,
                Some(format!(
                    "{in_place} de {} eslabones ya están en su lugar. Pregúntate qué tiene que haber ocurrido antes de cada paso.",
                    key.len()
                )),
            ))
        }
        Some("boundary_case_audit") | Some("boundary_case_Audit") => {
            let key: Vec<bool> = block
                .content_json
                .get("boundaryCases")
                .and_then(|v| v.as_array())
                .ok_or_else(|| missing("boundaryCases"))?
                .iter()
                .map(|c| c.get("followsRule").and_then(|v| v.as_bool()).unwrap_or(false))
                .collect();
            let Some(given) = answer.case_verdicts else { return Ok((false, None)) };
            if given.len() != key.len() {
                return Ok((false, None));
            }
            let right = given.iter().zip(&key).filter(|(g, k)| g == k).count();
            if right == key.len() {
                return Ok((true, None));
            }
            Ok((
                false,
                Some(format!(
                    "{right} de {} casos bien clasificados. Vuelve a mirar dónde está el borde exacto de la regla.",
                    key.len()
                )),
            ))
        }
        _ => {
            let correct = block
                .content_json
                .get("correctOption")
                .and_then(|v| v.as_str())
                .ok_or_else(|| missing("correctOption"))?;
            Ok((answer.selected_option == Some(correct), None))
        }
    }
}

pub(super) fn grade_branching_scenario(block: &NotebookBlock, selected_choice: &str) -> AppResult<bool> {
    let branches = block
        .content_json
        .get("branches")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Persistence("branching_scenario_challenge block is missing branches".to_string()))?;
    for branch in branches {
        if branch.get("choice").and_then(|v| v.as_str()) == Some(selected_choice) {
            return Ok(branch.get("isOptimal").and_then(|v| v.as_bool()).unwrap_or(false));
        }
    }
    // An unrecognized choice string never passes — defensive against a
    // stale/forged submission that doesn't match any real branch.
    Ok(false)
}

/// A free-text gate's verdict: `scaffold_feedback` is `None` on a pass;
/// `criteria` is the grader's per-criterion rubric judgment (may be empty).
pub(super) struct ModelGrade {
    pub passed: bool,
    pub scaffold_feedback: Option<String>,
    pub criteria: Vec<RubricCriterion>,
}

/// Fast-path for clearly-invalid free-text submissions (empty after trim,
/// identical to the template, or shorter than 10 chars). These never deserve
/// a model round trip and are the majority of accidental clicks.
pub(super) fn fast_path_reject(submission_text: &str, block: &NotebookBlock) -> Option<ModelGrade> {
    let trimmed = submission_text.trim();
    if trimmed.is_empty() || trimmed.len() < 10 {
        return Some(ModelGrade { passed: false, scaffold_feedback: Some("Escribe algo más sustancial antes de enviar.".to_string()), criteria: vec![] });
    }
    // If the student pasted the template unchanged (the cloze with ___ still there),
    // it's not a real attempt.
    if let Some(tpl) = block.content_json.get("workedTemplate").and_then(|v| v.as_str()) {
        if trimmed == tpl.trim() || trimmed.contains("___") && trimmed.split("___").count() > 1 {
            return Some(ModelGrade { passed: false, scaffold_feedback: Some("Modifica los huecos antes de enviar; no envíes la plantilla sin completar.".to_string()), criteria: vec![] });
        }
    }
    None
}

/// Grades a free-text submission (`heuristic_error_audit`'s diagnosis,
/// `hands_on_mission`'s solution) via `notebook_gate_grader`.
pub(super) async fn grade_with_model(
    orchestrator: &Orchestrator,
    app: Option<&AppHandle>,
    block: &NotebookBlock,
    submission_text: &str,
    attempt_number: u32,
    selected_line: Option<u32>,
) -> AppResult<ModelGrade> {
    run_gate_grader(orchestrator, app, render::render_grading_input(block, submission_text, attempt_number, selected_line)).await
}

/// Grades ONE written retrieval answer against the item's `expectedAnswer`
/// with the same grader agent (it has a `spaced_interleaved_retrieval` mode):
/// no scaffold on a miss, because the answer is revealed right after.
pub(super) async fn grade_retrieval_with_model(
    orchestrator: &Orchestrator,
    app: Option<&AppHandle>,
    item: &serde_json::Value,
    answer: &str,
) -> AppResult<ModelGrade> {
    run_gate_grader(orchestrator, app, render::render_retrieval_grading_input(item, answer)).await
}

async fn run_gate_grader(orchestrator: &Orchestrator, app: Option<&AppHandle>, base_input: String) -> AppResult<ModelGrade> {
    let mut note = String::new();
    let mut last_err: Option<AppError> = None;
    for attempt in 0..MAX_GRADING_ATTEMPTS {
        let capture: GateGradingCapture = Arc::new(Mutex::new(None));
        let input = format!("{base_input}{note}");
        match orchestrator.run_gate_grading_agent(app, NOTEBOOK_GATE_GRADER_AGENT_ID, &input, capture.clone()).await {
            Ok(output) => {
                let result = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(r) = result {
                    return Ok(ModelGrade { passed: r.passed, scaffold_feedback: r.scaffold.map(|s| s.content), criteria: r.criteria });
                }

                // Fallback 1: Parse JSON verdict directly from the model's text response
                if let Some(r) = parse_gate_grade_from_text(&output.text) {
                    tracing::info!("recovered gate grading result directly from model text output");
                    return Ok(ModelGrade { passed: r.passed, scaffold_feedback: r.scaffold.map(|s| s.content), criteria: r.criteria });
                }

                // Fallback 2: Retry with assertive system note if attempts remain
                if attempt + 1 < MAX_GRADING_ATTEMPTS {
                    tracing::warn!("grader returned without calling grade_gate_submission, retrying with explicit reminder");
                    note = "

NOTA_DEL_SISTEMA: No llamaste a la herramienta obligatoria grade_gate_submission.                             DEBES invocar la herramienta grade_gate_submission con los campos:                             passed (boolean), rationale (string) y scaffold (opcional).                             Ejecútala ahora."
                        .to_string();
                    continue;
                }

                return Err(AppError::AgentExecutionFailed(
                    "el calificador no llamó a grade_gate_submission".to_string(),
                ));
            }
            Err(e) if e.is_output_budget_exhausted() && attempt + 1 < MAX_GRADING_ATTEMPTS => {
                tracing::warn!(error = %e, "gate grading ran out of output budget, retrying more concisely");
                note = "\n\nNOTA_DEL_SISTEMA: la respuesta anterior se quedó sin espacio de salida. Sé mucho más \
                         breve: rationale en una frase, scaffold.content en 1-2 frases. Vuelve a llamar a \
                         grade_gate_submission en este turno."
                    .to_string();
            }
            Err(e) if e.is_transient() && attempt + 1 < MAX_GRADING_ATTEMPTS => {
                let backoff = RETRY_BACKOFF_BASE * (attempt as u32 + 1);
                tracing::warn!(error = %e, backoff_ms = backoff.as_millis(), "gate grading failed, retrying after backoff");
                tokio::time::sleep(backoff).await;
            }
            Err(e) => {
                last_err = Some(e);
                break;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::AgentExecutionFailed("el calificador de compuertas falló".to_string())))
}

/// Grades the closing block's free-text reflection via
/// `closure_feedback_grader`. Unlike gate scaffolds, `feedback` is ALWAYS
/// present (student-facing on pass and fail alike), so an empty one counts
/// as a grader contract violation, not as "no feedback".
pub(super) async fn grade_closure_with_model(
    orchestrator: &Orchestrator,
    app: Option<&AppHandle>,
    block: &NotebookBlock,
    reflection: &str,
    attempt_number: u32,
) -> AppResult<ClosureFeedbackResult> {
    let base_input = render::render_closure_grading_input(block, reflection, attempt_number);
    let mut note = String::new();
    let mut last_err: Option<AppError> = None;
    for attempt in 0..MAX_GRADING_ATTEMPTS {
        let capture: ClosureFeedbackCapture = Arc::new(Mutex::new(None));
        let input = format!("{base_input}{note}");
        match orchestrator.run_closure_grading_agent(app, CLOSURE_FEEDBACK_GRADER_AGENT_ID, &input, capture.clone()).await {
            Ok(_) => {
                let result = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
                return match result {
                    Some(r) if !r.feedback.trim().is_empty() => Ok(r),
                    Some(_) => Err(AppError::AgentExecutionFailed(
                        "el evaluador del cierre devolvió un feedback vacío".to_string(),
                    )),
                    None => Err(AppError::AgentExecutionFailed(
                        "el evaluador del cierre no llamó a grade_closure_submission".to_string(),
                    )),
                };
            }
            Err(e) if e.is_output_budget_exhausted() && attempt + 1 < MAX_GRADING_ATTEMPTS => {
                tracing::warn!(error = %e, "closure grading ran out of output budget, retrying more concisely");
                note = "\n\nNOTA_DEL_SISTEMA: la respuesta anterior se quedó sin espacio de salida. Sé mucho más \
                         breve: feedback en 2-3 frases. Vuelve a llamar a grade_closure_submission en este turno."
                    .to_string();
            }
            Err(e) if e.is_transient() && attempt + 1 < MAX_GRADING_ATTEMPTS => {
                let backoff = RETRY_BACKOFF_BASE * (attempt as u32 + 1);
                tracing::warn!(error = %e, backoff_ms = backoff.as_millis(), "closure grading failed, retrying after backoff");
                tokio::time::sleep(backoff).await;
            }
            Err(e) => {
                last_err = Some(e);
                break;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::AgentExecutionFailed("el evaluador del cierre falló".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(block_type: DynamicBlockType, status: BlockStatus, content: serde_json::Value) -> NotebookBlock {
        NotebookBlock {
            id: "b".to_string(),
            document_id: "d".to_string(),
            block_type,
            content_json: content,
            order_index: 0,
            status,
            concept_node_id: None,
            attempt_count: 0,
            last_feedback: None,
        }
    }

    fn steps(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn multiple_choice_is_the_default_mode() {
        let b = block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready, serde_json::json!({ "correctOption": "B" }));
        let ok = PredictionAnswer { selected_option: Some("B"), ..Default::default() };
        assert_eq!(grade_prediction_gate(&b, &ok).unwrap(), (true, None));
        let wrong = PredictionAnswer { selected_option: Some("A"), ..Default::default() };
        assert_eq!(grade_prediction_gate(&b, &wrong).unwrap(), (false, None));
        assert!(!grade_prediction_gate(&b, &PredictionAnswer::default()).unwrap().0, "no answer never passes");
    }

    #[test]
    fn causal_chain_requires_the_exact_order_and_reports_only_how_many_links_are_right() {
        let b = block(
            DynamicBlockType::InteractivePredictionGate,
            BlockStatus::Ready,
            serde_json::json!({ "interactionMode": "causal_chain_order", "correctOrder": ["A", "B", "C"] }),
        );
        let right = steps(&["A", "B", "C"]);
        assert_eq!(grade_prediction_gate(&b, &PredictionAnswer { ordered_steps: Some(&right), ..Default::default() }).unwrap(), (true, None));

        let wrong = steps(&["A", "C", "B"]);
        let (passed, hint) = grade_prediction_gate(&b, &PredictionAnswer { ordered_steps: Some(&wrong), ..Default::default() }).unwrap();
        assert!(!passed);
        let hint = hint.expect("partial-credit hint");
        assert!(hint.contains("1 de 3"), "{hint}");
        assert!(!hint.contains("A →") && !hint.contains("correctOrder"), "never reveals the order: {hint}");

        let short = steps(&["A", "B"]);
        assert!(!grade_prediction_gate(&b, &PredictionAnswer { ordered_steps: Some(&short), ..Default::default() }).unwrap().0);
    }

    #[test]
    fn boundary_cases_are_graded_against_the_stored_verdicts_in_both_spellings_of_the_mode() {
        for mode in ["boundary_case_audit", "boundary_case_Audit"] {
            let b = block(
                DynamicBlockType::InteractivePredictionGate,
                BlockStatus::Ready,
                serde_json::json!({ "interactionMode": mode, "boundaryCases": [
                    { "statement": "x", "followsRule": true }, { "statement": "y", "followsRule": false }
                ] }),
            );
            let right = [true, false];
            assert!(grade_prediction_gate(&b, &PredictionAnswer { case_verdicts: Some(&right), ..Default::default() }).unwrap().0);
            let wrong = [true, true];
            let (passed, hint) = grade_prediction_gate(&b, &PredictionAnswer { case_verdicts: Some(&wrong), ..Default::default() }).unwrap();
            assert!(!passed);
            assert!(hint.unwrap().contains("1 de 2"));
        }
    }

    #[test]
    fn redaction_strips_every_new_answer_key_until_the_gate_is_resolved() {
        let gate = block(
            DynamicBlockType::InteractivePredictionGate,
            BlockStatus::Ready,
            serde_json::json!({
                "correctOption": "B", "correctOrder": ["A", "B"], "shuffledSteps": ["B", "A"],
                "boundaryCases": [{ "statement": "x", "followsRule": true }]
            }),
        );
        let redacted = redact_block(&gate).content_json;
        assert!(redacted.get("correctOption").is_none() && redacted.get("correctOrder").is_none());
        assert!(redacted["boundaryCases"][0].get("followsRule").is_none());
        assert_eq!(redacted["shuffledSteps"], serde_json::json!(["B", "A"]), "the shuffled links ARE shown");
        assert_eq!(redacted["boundaryCases"][0]["statement"], "x");

        let audit_content = serde_json::json!({ "modelSolution": "m", "faultyLineNumber": 2 });
        let open = redact_block(&block(DynamicBlockType::HeuristicErrorAudit, BlockStatus::Failed, audit_content.clone())).content_json;
        assert!(open.get("modelSolution").is_none() && open.get("faultyLineNumber").is_none());
        let passed = redact_block(&block(DynamicBlockType::HeuristicErrorAudit, BlockStatus::Passed, audit_content)).content_json;
        assert_eq!(passed["faultyLineNumber"], 2, "revealed once the gate is passed");
    }
}


fn parse_gate_grade_from_text(text: &str) -> Option<GateGradingResult> {
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            let json_str = text[start + 7..start + 7 + end].trim();
            if let Ok(res) = serde_json::from_str::<GateGradingResult>(json_str) {
                return Some(res);
            }
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let candidate = &text[start..=end];
                if let Ok(res) = serde_json::from_str::<GateGradingResult>(candidate) {
                    return Some(res);
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                    if let Some(passed) = val.get("passed").and_then(|v| v.as_bool()) {
                        let rationale = val.get("rationale").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        return Some(GateGradingResult {
                            passed,
                            rationale,
                            criteria: Vec::new(),
                            scaffold: None,
                        });
                    }
                }
            }
        }
    }

    let lower = text.to_lowercase();
    let indicates_pass = lower.contains(r#""passed": true"#)
        || lower.contains("passed: true")
        || lower.contains("es correcta")
        || lower.contains("aprobado")
        || lower.contains("cumple con los requisitos")
        || lower.contains("solución correcta");

    let indicates_fail = lower.contains(r#""passed": false"#)
        || lower.contains("passed: false")
        || lower.contains("es incorrecta")
        || lower.contains("no cumple")
        || lower.contains("reprobado")
        || lower.contains("faltan");

    if indicates_pass && !indicates_fail {
        return Some(GateGradingResult {
            passed: true,
            rationale: text.chars().take(200).collect(),
            criteria: Vec::new(),
            scaffold: None,
        });
    } else if indicates_fail && !indicates_pass {
        return Some(GateGradingResult {
            passed: false,
            rationale: text.chars().take(200).collect(),
            criteria: Vec::new(),
            scaffold: Some(crate::tools::GateScaffold {
                scaffold_type: crate::tools::ScaffoldType::SocraticHint,
                content: text.chars().take(300).collect(),
            }),
        });
    }

    None
}
