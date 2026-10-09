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
use crate::tools::{ClosureFeedbackCapture, ClosureFeedbackResult, GateGradingCapture};

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

pub(super) fn grade_prediction_gate(block: &NotebookBlock, selected_option: &str) -> AppResult<bool> {
    let correct = block
        .content_json
        .get("correctOption")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Persistence("interactive_prediction_gate block is missing correctOption".to_string()))?;
    Ok(correct == selected_option)
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

/// Grades a free-text submission (`heuristic_error_audit`'s diagnosis,
/// `hands_on_mission`'s solution) via `notebook_gate_grader`.
pub(super) async fn grade_with_model(
    orchestrator: &Orchestrator,
    app: Option<&AppHandle>,
    block: &NotebookBlock,
    submission_text: &str,
    attempt_number: u32,
) -> AppResult<ModelGrade> {
    run_gate_grader(orchestrator, app, render::render_grading_input(block, submission_text, attempt_number)).await
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
            Ok(_) => {
                let result = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
                return match result {
                    Some(r) => Ok(ModelGrade { passed: r.passed, scaffold_feedback: r.scaffold.map(|s| s.content), criteria: r.criteria }),
                    None => Err(AppError::AgentExecutionFailed(
                        "el calificador no llamó a grade_gate_submission".to_string(),
                    )),
                };
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
