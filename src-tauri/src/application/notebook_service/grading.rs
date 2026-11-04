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
use crate::tools::{ClosureFeedbackCapture, ClosureFeedbackResult, GateGradingCapture, GateGradingResult, GateScaffold, ScaffoldType};

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
                    let feedback = r.scaffold.map(|s| clean_tool_and_xml_tags(&s.content));
                    return Ok(ModelGrade { passed: r.passed, scaffold_feedback: feedback, criteria: r.criteria });
                }

                // Fallback 1: Parse verdict directly from model text response (handles XML tool calls & JSON)
                if let Some(r) = parse_gate_grade_from_text(&output.text) {
                    tracing::info!("recovered gate grading result directly from model text output");
                    let feedback = r.scaffold.map(|s| clean_tool_and_xml_tags(&s.content));
                    return Ok(ModelGrade { passed: r.passed, scaffold_feedback: feedback, criteria: r.criteria });
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
            Ok(output) => {
                let result = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(mut r) = result {
                    r.feedback = clean_tool_and_xml_tags(&r.feedback);
                    if !r.feedback.trim().is_empty() {
                        return Ok(r);
                    }
                }
                if let Some(mut r) = parse_closure_grade_from_text(&output.text) {
                    r.feedback = clean_tool_and_xml_tags(&r.feedback);
                    if !r.feedback.trim().is_empty() {
                        return Ok(r);
                    }
                }
                return Err(AppError::AgentExecutionFailed(
                    "el evaluador del cierre no llamó a grade_closure_submission".to_string(),
                ));
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

    #[test]
    fn parses_xml_tool_calls_and_strips_tool_markup() {
        let raw_user_sample = "<tool_call><function=grade_gate_submission><parameter=passed>false</parameter><parameter=rationale>La tabla contiene los valores correctos (16/24 en Default, 20.0/30.0 en Relaxed) pero incumple tres criterios centrales de la rúbrica: (1) falta la derivación escrita del factor de Relaxed con la opera";
        let parsed = parse_gate_grade_from_text(raw_user_sample).expect("should parse xml tool call");
        assert!(!parsed.passed);
        assert!(!parsed.rationale.contains("<tool_call"));
        assert!(!parsed.rationale.contains("<parameter"));
        assert!(parsed.rationale.contains("La tabla contiene los valores correctos"));

        let scaffold = parsed.scaffold.expect("failure should provide scaffold feedback");
        assert!(!scaffold.content.contains("<tool_call"));
        assert!(!scaffold.content.contains("<parameter"));
        assert!(scaffold.content.contains("La tabla contiene los valores correctos"));
    }

    #[test]
    fn cleans_nested_thought_and_function_tags() {
        let text = "<thought>Checking criteria...</thought><tool_call><function=grade_gate_submission><parameter=passed>true</parameter><parameter=rationale>¡Excelente solución!</parameter></tool_call>";
        let parsed = parse_gate_grade_from_text(text).expect("should parse pass");
        assert!(parsed.passed);
        assert_eq!(parsed.rationale, "¡Excelente solución!");
    }

    #[test]
    fn unwraps_raw_json_scaffold_payload_into_clean_human_feedback() {
        let raw_json_feedback = r#"{"scaffoldType": "worked_example", "content": "Tu componente ya sigue la idea central: ningún número escrito a mano, todo pasa por var(...). Un caso análogo con otro contrato de nombres: Contrato ajeno: --brand-font-h*, --brand-gap-*, --brand-h-control*. ```jsx const Chip = () => ( <Etiqueta /> ); ```"}"#;
        let cleaned = clean_tool_and_xml_tags(raw_json_feedback);
        assert!(!cleaned.contains("scaffoldType"));
        assert!(!cleaned.starts_with('{'));
        assert!(cleaned.contains("Tu componente ya sigue la idea central"));
        assert!(cleaned.contains("<Etiqueta />"));
    }

    #[test]
    fn parses_xml_tool_call_with_json_scaffold() {
        let xml_sample = r#"<tool_call><function=grade_gate_submission><parameter=passed>false</parameter><parameter=rationale>Faltan tokens de escala</parameter><parameter=scaffold>{"scaffoldType": "worked_example", "content": "Usa var(--space-md) en vez de 12px."}</parameter></tool_call>"#;
        let parsed = parse_gate_grade_from_text(xml_sample).expect("should parse tool call with json scaffold");
        assert!(!parsed.passed);
        let scaffold = parsed.scaffold.expect("must contain scaffold");
        assert_eq!(scaffold.content, "Usa var(--space-md) en vez de 12px.");
    }
}


/// Unwraps JSON object strings that carry scaffold / content / feedback payloads,
/// extracting the student-facing human prose.
fn unwrap_json_payload(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.contains('{') || !trimmed.contains('}') {
        return None;
    }
    if !trimmed.contains("\"content\"")
        && !trimmed.contains("\"feedback\"")
        && !trimmed.contains("\"scaffold\"")
        && !trimmed.contains("\"scaffoldType\"")
    {
        return None;
    }

    let first = trimmed.find('{')?;
    let last = trimmed.rfind('}')?;
    if last <= first {
        return None;
    }

    let candidate = &trimmed[first..=last];
    let val_opt = serde_json::from_str::<serde_json::Value>(candidate)
        .ok()
        .or_else(|| crate::tools::json_repair::parse_repaired_value(candidate).ok());

    if let Some(val) = val_opt {
        let target = val.get("arguments").unwrap_or(&val);
        if let Some(c) = target.get("content").and_then(|v| v.as_str()) {
            return Some(c.to_string());
        }
        if let Some(f) = target.get("feedback").and_then(|v| v.as_str()) {
            return Some(f.to_string());
        }
        if let Some(sc) = target.get("scaffold") {
            if let Some(c) = sc.get("content").and_then(|v| v.as_str()) {
                return Some(c.to_string());
            } else if let Some(s) = sc.as_str() {
                let nested_opt = serde_json::from_str::<serde_json::Value>(s)
                    .ok()
                    .or_else(|| crate::tools::json_repair::parse_repaired_value(s).ok());
                if let Some(nested) = nested_opt {
                    if let Some(c) = nested.get("content").and_then(|v| v.as_str()) {
                        return Some(c.to_string());
                    }
                }
                return Some(s.to_string());
            }
        }
        if let Some(r) = target.get("rationale").and_then(|v| v.as_str()) {
            if !r.trim().is_empty() {
                return Some(r.to_string());
            }
        }
    }
    None
}

/// Strips out XML tool-calling wrappers (<tool_call>, <function=...>, <parameter=...>,
/// </parameter>, <invoke...>, <thought>...</thought>, etc.), unwraps serialized JSON
/// scaffold/feedback payloads, and ensures student-facing messages and feedbacks are
/// always clean human-readable prose.
pub fn clean_tool_and_xml_tags(text: &str) -> String {
    // 0. Unwrap serialized JSON object if the string is or contains one
    if let Some(unwrapped) = unwrap_json_payload(text) {
        return clean_tool_and_xml_tags(&unwrapped);
    }

    let mut s = text.to_string();

    // 1. Remove <thought>...</thought> and <thinking>...</thinking>
    while let Some(start) = s.find("<thought>") {
        if let Some(end) = s[start..].find("</thought>") {
            s.replace_range(start..start + end + "</thought>".len(), "");
        } else {
            s.truncate(start);
            break;
        }
    }
    while let Some(start) = s.find("<thinking>") {
        if let Some(end) = s[start..].find("</thinking>") {
            s.replace_range(start..start + end + "</thinking>".len(), "");
        } else {
            s.truncate(start);
            break;
        }
    }

    // 2. Strip any <parameter=passed>...</parameter> or <parameter name="passed">...</parameter>
    // so verdicts don't get glued to the feedback text.
    for param in &["passed", "function", "name"] {
        for prefix in &[
            format!("<parameter={param}>"),
            format!("<parameter name=\"{param}\">"),
            format!("<{param}>"),
        ] {
            while let Some(start) = s.find(prefix.as_str()) {
                let after = start + prefix.len();
                let end = s[after..]
                    .find("</parameter>")
                    .or_else(|| s[after..].find(format!("</{param}>").as_str()))
                    .map(|idx| after + idx + "</parameter>".len())
                    .unwrap_or(after);
                s.replace_range(start..end, "");
            }
        }
    }

    // 3. Strip all tool-calling XML tags (<tool_call>, </tool_call>, <parameter...>, </parameter>, <function...>, etc.)
    // while preserving student prose, JSX tags, and language generics (<Etiqueta />, Vec<T>).
    let mut cleaned = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'<' {
            let rest = &s[i + 1..];
            let tool_prefixes = [
                "tool_call", "/tool_call",
                "function", "/function",
                "parameter", "/parameter",
                "invoke", "/invoke",
                "thought", "/thought",
                "thinking", "/thinking",
                "tool_response", "/tool_response",
                "call:", "/call:",
            ];
            let is_tool_tag = tool_prefixes.iter().any(|&p| {
                if rest.starts_with(p) {
                    let after = &rest[p.len()..];
                    after.is_empty() || after.starts_with('>') || after.starts_with(' ') || after.starts_with('=') || after.starts_with('/')
                } else {
                    false
                }
            });
            if is_tool_tag {
                if let Some(end_rel) = rest.find('>') {
                    i += 1 + end_rel + 1;
                    continue;
                }
            }
        }
        if let Some(ch) = s[i..].chars().next() {
            cleaned.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    // 4. Unwrap markdown code fence if the entire string was wrapped in ```
    let trimmed = cleaned.trim();
    let unwrapped = if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let inner = trimmed.trim_start_matches('`');
        let inner = if let Some(idx) = inner.find('\n') {
            &inner[idx + 1..]
        } else {
            inner
        };
        inner.trim_end_matches('`').trim()
    } else {
        trimmed
    };

    // 5. If tag-stripping revealed a JSON payload, unwrap it:
    if let Some(json_content) = unwrap_json_payload(unwrapped) {
        return clean_tool_and_xml_tags(&json_content);
    }

    unwrapped.to_string()
}

fn parse_xml_param(text: &str, name: &str) -> Option<String> {
    let patterns = [
        format!("<parameter={name}>"),
        format!("<parameter name=\"{name}\">"),
        format!("<{name}>"),
    ];
    for pat in &patterns {
        if let Some(idx) = text.find(pat.as_str()) {
            let start = idx + pat.len();
            let end_markers = [
                "</parameter>",
                "</tool_call>",
                "</function>",
                "</invoke>",
                "<parameter",
                "<function",
            ];
            let mut closest_end = text.len() - start;
            for marker in &end_markers {
                if let Some(m_idx) = text[start..].find(marker) {
                    if m_idx < closest_end {
                        closest_end = m_idx;
                    }
                }
            }
            let val = text[start..start + closest_end].trim();
            let cleaned = clean_tool_and_xml_tags(val);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

fn parse_gate_grade_from_text(text: &str) -> Option<GateGradingResult> {
    // 1. Try parsing XML-style tool calls (<tool_call><function=grade_gate_submission>...)
    if text.contains("<tool_call")
        || text.contains("<function")
        || text.contains("<parameter")
        || text.contains("<invoke")
    {
        let passed_str = parse_xml_param(text, "passed");
        let passed = passed_str.as_ref().map(|s| s.eq_ignore_ascii_case("true"));
        let rationale = parse_xml_param(text, "rationale").unwrap_or_default();
        let scaffold_content = parse_xml_param(text, "scaffold")
            .or_else(|| parse_xml_param(text, "content"));

        if let Some(is_pass) = passed {
            let scaffold = if is_pass {
                None
            } else {
                let content = scaffold_content
                    .or_else(|| {
                        if !rationale.is_empty() {
                            Some(rationale.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        "Revisa los requisitos del reto e inténtalo de nuevo.".to_string()
                    });
                Some(GateScaffold {
                    scaffold_type: ScaffoldType::SocraticHint,
                    content,
                })
            };
            return Some(GateGradingResult {
                passed: is_pass,
                rationale,
                criteria: Vec::new(),
                scaffold,
            });
        }

        // Even if passed param was missing, check if text/rationale indicates pass or fail
        let lower = text.to_lowercase();
        let indicates_pass = lower.contains("passed: true")
            || lower.contains("\"passed\": true")
            || lower.contains("es correcta")
            || lower.contains("aprobado");
        let indicates_fail = lower.contains("passed: false")
            || lower.contains("\"passed\": false")
            || lower.contains("es incorrecta")
            || lower.contains("no cumple")
            || lower.contains("incumple")
            || lower.contains("reprobado")
            || lower.contains("faltan");

        if indicates_fail && !indicates_pass {
            let content = if !rationale.is_empty() {
                rationale.clone()
            } else {
                clean_tool_and_xml_tags(text)
            };
            return Some(GateGradingResult {
                passed: false,
                rationale,
                criteria: Vec::new(),
                scaffold: Some(GateScaffold {
                    scaffold_type: ScaffoldType::SocraticHint,
                    content,
                }),
            });
        } else if indicates_pass && !indicates_fail {
            return Some(GateGradingResult {
                passed: true,
                rationale,
                criteria: Vec::new(),
                scaffold: None,
            });
        }
    }

    // 2. Try JSON in code fences
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            let json_str = text[start + 7..start + 7 + end].trim();
            if let Ok(res) = serde_json::from_str::<GateGradingResult>(json_str) {
                return Some(res);
            }
        }
    }

    // 3. Try balanced JSON object
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let candidate = &text[start..=end];
                if let Ok(res) = serde_json::from_str::<GateGradingResult>(candidate) {
                    return Some(res);
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                    let target_val = val.get("arguments").unwrap_or(&val);
                    if let Some(passed) = target_val.get("passed").and_then(|v| v.as_bool()) {
                        let rationale = target_val
                            .get("rationale")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let scaffold = if passed {
                            None
                        } else {
                            let scaffold_txt = target_val
                                .get("scaffold")
                                .and_then(|s| {
                                    if let Some(c) = s.get("content").and_then(|c| c.as_str()) {
                                        Some(c.to_string())
                                    } else if let Some(str_val) = s.as_str() {
                                        if let Ok(nested) = serde_json::from_str::<serde_json::Value>(str_val) {
                                            if let Some(c) = nested.get("content").and_then(|c| c.as_str()) {
                                                return Some(c.to_string());
                                            }
                                        }
                                        Some(str_val.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .or_else(|| {
                                    if !rationale.is_empty() {
                                        Some(rationale.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| {
                                    "Revisa tu entrega e inténtalo de nuevo.".to_string()
                                });
                            Some(GateScaffold {
                                scaffold_type: ScaffoldType::SocraticHint,
                                content: clean_tool_and_xml_tags(&scaffold_txt),
                            })
                        };
                        return Some(GateGradingResult {
                            passed,
                            rationale: clean_tool_and_xml_tags(&rationale),
                            criteria: Vec::new(),
                            scaffold,
                        });
                    }
                }
            }
        }
    }

    // 4. Heuristic text analysis with STRICT tag cleaning
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
        || lower.contains("incumple")
        || lower.contains("reprobado")
        || lower.contains("faltan");

    let cleaned_text = clean_tool_and_xml_tags(text);

    if indicates_pass && !indicates_fail {
        return Some(GateGradingResult {
            passed: true,
            rationale: cleaned_text.chars().take(200).collect(),
            criteria: Vec::new(),
            scaffold: None,
        });
    } else if indicates_fail && !indicates_pass {
        let content: String = if !cleaned_text.is_empty() {
            cleaned_text.chars().take(300).collect()
        } else {
            "Revisa los requisitos del reto e inténtalo de nuevo.".to_string()
        };
        return Some(GateGradingResult {
            passed: false,
            rationale: content.chars().take(200).collect(),
            criteria: Vec::new(),
            scaffold: Some(crate::tools::GateScaffold {
                scaffold_type: crate::tools::ScaffoldType::SocraticHint,
                content,
            }),
        });
    }

    None
}

fn parse_closure_grade_from_text(text: &str) -> Option<ClosureFeedbackResult> {
    if text.contains("<tool_call")
        || text.contains("<function")
        || text.contains("<parameter")
        || text.contains("<invoke")
    {
        let passed_str = parse_xml_param(text, "passed");
        let passed = passed_str
            .as_ref()
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        let feedback = parse_xml_param(text, "feedback")
            .unwrap_or_else(|| clean_tool_and_xml_tags(text));
        if !feedback.is_empty() {
            return Some(ClosureFeedbackResult { passed, feedback });
        }
    }

    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let candidate = &text[start..=end];
                if let Ok(res) = serde_json::from_str::<ClosureFeedbackResult>(candidate) {
                    return Some(ClosureFeedbackResult {
                        passed: res.passed,
                        feedback: clean_tool_and_xml_tags(&res.feedback),
                    });
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                    let target_val = val.get("arguments").unwrap_or(&val);
                    if let Some(feedback) = target_val.get("feedback").and_then(|v| v.as_str()) {
                        let passed = target_val.get("passed").and_then(|v| v.as_bool()).unwrap_or(true);
                        return Some(ClosureFeedbackResult {
                            passed,
                            feedback: clean_tool_and_xml_tags(feedback),
                        });
                    }
                }
            }
        }
    }

    let cleaned = clean_tool_and_xml_tags(text);
    if !cleaned.is_empty() {
        return Some(ClosureFeedbackResult {
            passed: true,
            feedback: cleaned,
        });
    }

    None
}
