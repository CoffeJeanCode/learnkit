use std::sync::Arc;

use uuid::Uuid;

use crate::agents::roadmap_agent::ROADMAP_AGENT_ID;
use crate::domain::roadmap::{
    parse_turn_envelope, ApprovalRecord, DiagnosticAssessmentSummary, DraftSyllabusTree, LearnerProfileState,
    NextAction, PackageStatus, RevisionEntry, RoadmapPackageFinal, RoadmapPhase, RoadmapSession, RoadmapTurnEnvelope,
    SessionStatus,
};
use crate::error::{AppError, AppResult};
use crate::orchestration::Orchestrator;
use crate::persistence::FileStore;

/// How many unresolved clarification rounds on the same phase before the
/// service forces the agent to stop re-asking and assume a default. This is
/// a hard backstop independent of whether the model follows its own
/// "reformulate once, then assume" instruction (see the system prompt) — the
/// prompt can be ignored, the gate in code cannot.
const CLARIFICATION_CEILING: u8 = 2;

#[derive(Debug)]
pub struct RoadmapTurnResult {
    pub message: String,
    pub session: RoadmapSession,
}

/// Application service for the Roadmap & Syllabus Diagnostic Agent.
///
/// Bridges the stateless [`Orchestrator`] (one `input` string in, one `text`
/// out, no memory) with a persisted, multi-turn [`RoadmapSession`]. Every
/// turn: render context into `input` -> run the agent -> parse the
/// `roadmap_state` block it must emit -> re-validate the phase gate in code
/// -> persist. The gate decision never trusts the model's `next_action`
/// alone; it is re-derived from the `dod` map every time.
pub struct RoadmapService {
    orchestrator: Arc<Orchestrator>,
    store: FileStore,
}

impl RoadmapService {
    pub fn new(orchestrator: Arc<Orchestrator>, store: FileStore) -> Self {
        Self { orchestrator, store }
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<RoadmapSession> {
        self.store
            .load_roadmap_session(session_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("roadmap session not found: {session_id}")))
    }

    pub async fn start_session(&self, app: Option<&tauri::AppHandle>) -> AppResult<RoadmapTurnResult> {
        let session = RoadmapSession::new(Uuid::new_v4().to_string(), now_ms());
        self.store.save_roadmap_session(&session)?;
        self.advance(
            app,
            session,
            "(El estudiante acaba de abrir la sesión. Dale la bienvenida y formula tu primera \
             pregunta de la Fase 1.)",
        )
        .await
    }

    #[tracing::instrument(skip(self, app, user_message), fields(session_id = %session_id))]
    pub async fn send_message(
        &self,
        app: Option<&tauri::AppHandle>,
        session_id: &str,
        user_message: &str,
    ) -> AppResult<RoadmapTurnResult> {
        let session = self.get_session(session_id)?;
        self.advance(app, session, user_message).await
    }

    async fn advance(
        &self,
        app: Option<&tauri::AppHandle>,
        mut session: RoadmapSession,
        user_message: &str,
    ) -> AppResult<RoadmapTurnResult> {
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }

        let base_input = render_turn_input(&session, user_message);
        let (raw_text, envelope) = self.run_turn_with_retry(app, &base_input).await?;

        if envelope.session_id != session.session_id {
            tracing::warn!(
                expected = %session.session_id,
                got = %envelope.session_id,
                "roadmap agent echoed a mismatched session_id in its state block"
            );
        }

        self.apply_envelope(&mut session, &envelope);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;

        Ok(RoadmapTurnResult { message: strip_state_block(&raw_text), session })
    }

    /// Runs one turn; if the `roadmap_state` block is missing or malformed,
    /// retries exactly once with a corrective note appended to the same
    /// context (never surfaced to the learner as a broken turn).
    async fn run_turn_with_retry(
        &self,
        app: Option<&tauri::AppHandle>,
        base_input: &str,
    ) -> AppResult<(String, RoadmapTurnEnvelope)> {
        let mut input = base_input.to_string();
        for attempt in 0..2 {
            let output = self.orchestrator.run_agent(app, ROADMAP_AGENT_ID, &input, None).await?;
            match parse_turn_envelope(&output.text) {
                Ok(envelope) => return Ok((output.text, envelope)),
                Err(e) if attempt == 0 => {
                    tracing::warn!(error = %e, "roadmap_state parse failed, retrying once");
                    input = format!(
                        "{base_input}\n\nNOTA_DEL_SISTEMA: tu turno anterior no incluía un bloque \
                         ```roadmap_state``` JSON válido ({e}). Responde de nuevo, incluyendo el \
                         bloque correctamente formado según el contrato de salida."
                    );
                }
                Err(e) => {
                    return Err(AppError::AgentExecutionFailed(format!(
                        "roadmap_state inválido tras reintento: {e}"
                    )));
                }
            }
        }
        unreachable!("loop returns within its 2 attempts")
    }

    /// Merges the turn's patch, then re-derives the gate decision from `dod`
    /// in code — the model's own `next_action` is a proposal, not a grant.
    fn apply_envelope(&self, session: &mut RoadmapSession, envelope: &RoadmapTurnEnvelope) {
        session.merge_patch(envelope.phase, envelope.state_patch.clone());

        if let Some(target_phase) = envelope.flags.revision_requested {
            session.revision_log.push(RevisionEntry {
                at_turn: session.turn_count_in_phase,
                reverted_to_phase: target_phase,
                reason: "learner requested revision mid-flow".to_string(),
                fields_invalidated: fields_invalidated_by_revision(target_phase),
            });
            session.phase = target_phase;
            session.turn_count_in_phase = 0;
            session.dod.clear();
            return;
        }

        session.dod = envelope.dod.clone();

        let phase_key = phase_key(envelope.phase);
        if envelope.next_action == NextAction::RequestClarification {
            *session.clarification_attempts.entry(phase_key.to_string()).or_insert(0) += 1;
        }

        let all_dod_true = !envelope.dod.is_empty() && envelope.dod.values().all(|v| *v);
        if !all_dod_true {
            session.turn_count_in_phase += 1;
            return;
        }

        match envelope.next_action {
            NextAction::PhaseComplete => {
                if let Some(next) = envelope.phase.next() {
                    session.phase = next;
                    session.turn_count_in_phase = 0;
                    session.dod.clear();
                    session.clarification_attempts.remove(phase_key);
                }
            }
            NextAction::FinalDelivery if envelope.phase == RoadmapPhase::Negotiation => {
                match try_build_package(session) {
                    Ok(package) => {
                        session.final_package = Some(package);
                        session.status = SessionStatus::Sealed;
                    }
                    Err(reason) => {
                        tracing::warn!(
                            session_id = %session.session_id,
                            reason = %reason,
                            "roadmap package incomplete at final_delivery, staying in negotiation"
                        );
                        session.turn_count_in_phase += 1;
                    }
                }
            }
            _ => {
                session.turn_count_in_phase += 1;
            }
        }
    }
}

fn phase_key(phase: RoadmapPhase) -> &'static str {
    match phase {
        RoadmapPhase::Exploration => "exploration",
        RoadmapPhase::Diagnostic => "diagnostic",
        RoadmapPhase::Syllabus => "syllabus",
        RoadmapPhase::Negotiation => "negotiation",
    }
}

fn fields_invalidated_by_revision(target_phase: RoadmapPhase) -> Vec<String> {
    match target_phase {
        RoadmapPhase::Exploration => {
            vec!["diagnostic_summary".to_string(), "syllabus".to_string(), "negotiation".to_string()]
        }
        RoadmapPhase::Diagnostic => vec!["syllabus".to_string(), "negotiation".to_string()],
        RoadmapPhase::Syllabus => vec!["negotiation".to_string()],
        RoadmapPhase::Negotiation => vec![],
    }
}

/// Builds the compact, machine-readable context sent as `input` on every
/// turn. Deliberately does NOT resend full conversational history (the
/// backend keeps none) — only the accumulated deliverable buckets, current
/// phase/DoD, and any standing system directive (e.g. the clarification
/// backstop below).
fn render_turn_input(session: &RoadmapSession, user_message: &str) -> String {
    let mut directives: Vec<&str> = Vec::new();
    let attempts = session.clarification_attempts.get(phase_key(session.phase)).copied().unwrap_or(0);
    if attempts >= CLARIFICATION_CEILING {
        directives.push(
            "Este dato lleva varios intentos sin resolverse con claridad: asume un valor \
             razonable, decláralo en flags.assumed_fields, y avanza sin volver a preguntarlo.",
        );
    }

    let context = serde_json::json!({
        "session_id": session.session_id,
        "phase": session.phase,
        "turn_count_in_phase": session.turn_count_in_phase,
        "dod_last_known": session.dod,
        "learner_profile_so_far": session.learner_profile,
        "diagnostic_summary_so_far": session.diagnostic_summary,
        "syllabus_so_far": session.syllabus,
        "negotiation_so_far": session.negotiation,
        "system_directives": directives,
    });

    format!(
        "CONTEXTO_DE_SESION (uso interno, no lo muestres al estudiante):\n{}\n\nMENSAJE_DEL_ESTUDIANTE:\n{}",
        serde_json::to_string_pretty(&context).unwrap_or_default(),
        user_message
    )
}

fn strip_state_block(text: &str) -> String {
    const FENCE_OPEN: &str = "```roadmap_state";
    match text.find(FENCE_OPEN) {
        Some(idx) => text[..idx].trim_end().to_string(),
        None => text.trim_end().to_string(),
    }
}

/// Attempts to assemble the Phase-4 contract from what has accumulated so
/// far. Returns `Err(reason)` — never panics or silently seals a partial
/// package — when any required field is missing; `apply_envelope` treats
/// that as "not actually done yet" and keeps the session in `Negotiation`.
fn try_build_package(session: &RoadmapSession) -> Result<RoadmapPackageFinal, String> {
    let learner_profile: LearnerProfileState =
        serde_json::from_value(session.learner_profile.clone()).map_err(|e| format!("learner_profile incompleto: {e}"))?;
    let diagnostic_summary: DiagnosticAssessmentSummary = serde_json::from_value(session.diagnostic_summary.clone())
        .map_err(|e| format!("diagnostic_summary incompleto: {e}"))?;
    let syllabus: DraftSyllabusTree =
        serde_json::from_value(session.syllabus.clone()).map_err(|e| format!("syllabus incompleto: {e}"))?;
    if syllabus.milestones.is_empty() {
        return Err("syllabus sin hitos".to_string());
    }

    let executive_summary = session
        .negotiation
        .get("executive_summary")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "executive_summary ausente".to_string())?
        .to_string();

    let approval: ApprovalRecord = session
        .negotiation
        .get("approval")
        .cloned()
        .ok_or_else(|| "approval ausente".to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| format!("approval incompleto: {e}")))?;
    if approval.approved_verbatim_quote.trim().is_empty() {
        return Err("approved_verbatim_quote vacío".to_string());
    }

    let handoff_notes = session
        .negotiation
        .get("handoff_notes_for_notebook_builder")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "handoff_notes_for_notebook_builder ausente".to_string())?
        .to_string();

    Ok(RoadmapPackageFinal {
        schema_version: "1.0".to_string(),
        package_id: Uuid::new_v4().to_string(),
        session_id: session.session_id.clone(),
        generated_at_ms: now_ms(),
        status: PackageStatus::Approved,
        learner_profile,
        diagnostic_summary,
        syllabus,
        executive_summary,
        approval,
        revision_log: session.revision_log.clone(),
        handoff_notes_for_notebook_builder: handoff_notes,
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::agents::AgentRegistry;
    use crate::orchestration::PromptRunner;
    use crate::providers::factory::PromptOutput;

    /// Replays a fixed sequence of raw agent responses, ignoring `input`.
    /// Deterministic stand-in for the LLM so gate logic can be tested
    /// without network access (mirrors `MockRunner` in `orchestrator.rs`).
    struct ScriptedRunner {
        responses: Mutex<VecDeque<String>>,
    }

    impl ScriptedRunner {
        fn new(responses: Vec<&str>) -> Self {
            Self { responses: Mutex::new(responses.into_iter().map(str::to_string).collect()) }
        }
    }

    #[async_trait]
    impl PromptRunner for ScriptedRunner {
        async fn run(
            &self,
            _provider_id: &str,
            _model: &str,
            _system_prompt: &str,
            _input: &str,
            _tools: &[String],
        ) -> AppResult<PromptOutput> {
            let text = self.responses.lock().unwrap().pop_front().expect("scripted response available");
            Ok(PromptOutput { text, tool_calls: vec![] })
        }
    }

    fn service_with(responses: Vec<&str>) -> (RoadmapService, std::path::PathBuf) {
        let agents = Arc::new(AgentRegistry::new());
        agents.register(crate::agents::roadmap_agent::definition()).expect("register roadmap agent");
        let orchestrator = Arc::new(Orchestrator::new(agents, Arc::new(ScriptedRunner::new(responses))));
        let dir = std::env::temp_dir().join(format!("learnkit-roadmap-{}", Uuid::new_v4()));
        let store = FileStore::new(dir.clone());
        (RoadmapService::new(orchestrator, store), dir)
    }

    fn envelope_text(message: &str, phase: &str, dod: &str, patch: &str, next_action: &str) -> String {
        format!(
            "{message}\n\n```roadmap_state\n{{\"schema_version\":\"1.0\",\"session_id\":\"SID\",\"phase\":\"{phase}\",\
             \"dod\":{dod},\"state_patch\":{patch},\"next_action\":\"{next_action}\",\
             \"flags\":{{\"clarification_attempts\":0,\"assumed_fields\":[],\"revision_requested\":null}}}}\n```"
        )
    }

    #[tokio::test]
    async fn incomplete_dod_keeps_same_phase_and_strips_state_block() {
        let text = envelope_text(
            "¿Qué tema te gustaría aprender?",
            "exploration",
            r#"{"objetivo_terminal_confirmado": false}"#,
            "{}",
            "ask_question",
        );
        let (service, dir) = service_with(vec![&text]);
        let result = service.start_session(None).await.expect("turn ok");

        assert_eq!(result.session.phase, RoadmapPhase::Exploration);
        assert_eq!(result.session.turn_count_in_phase, 1);
        assert!(!result.message.contains("roadmap_state"));
        assert!(result.message.contains("¿Qué tema"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn complete_dod_with_phase_complete_advances_phase_and_resets_counters() {
        let text = envelope_text(
            "Perfecto, pasemos al diagnóstico.",
            "exploration",
            r#"{"objetivo_terminal_confirmado": true, "disponibilidad_horaria_confirmada": true}"#,
            r#"{"declared_topic": "Rust"}"#,
            "phase_complete",
        );
        let (service, dir) = service_with(vec![&text]);
        let result = service.start_session(None).await.expect("turn ok");

        assert_eq!(result.session.phase, RoadmapPhase::Diagnostic);
        assert_eq!(result.session.turn_count_in_phase, 0);
        assert!(result.session.dod.is_empty(), "dod resets when entering a new phase");
        assert_eq!(result.session.learner_profile["declared_topic"], "Rust");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn malformed_block_is_retried_once_then_succeeds() {
        let broken = "Hola\n```roadmap_state\n{not json\n```".to_string();
        let fixed = envelope_text("Hola de nuevo", "exploration", "{}", "{}", "ask_question");
        let (service, dir) = service_with(vec![&broken, &fixed]);
        let result = service.start_session(None).await.expect("recovers after one retry");
        assert!(result.message.contains("Hola de nuevo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn malformed_block_twice_fails_the_turn() {
        let broken = "no block here".to_string();
        let (service, dir) = service_with(vec![&broken, &broken]);
        let err = service.start_session(None).await.expect_err("must fail after 2 bad attempts");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn final_delivery_without_negotiation_fields_stays_in_negotiation() {
        let text = envelope_text(
            "¿Confirmas el plan?",
            "negotiation",
            r#"{"aprobacion_formal_recibida": true, "payload_sin_campos_vacios": true, "contexto_autosuficiente_para_notebook_builder": true}"#,
            "{}",
            "final_delivery",
        );
        let (service, dir) = service_with(vec![&text]);
        let result = service.start_session(None).await.expect("turn ok");

        // dod said "done", but negotiation bucket has no executive_summary/approval/handoff
        // notes yet -> service must refuse to seal, matching "no empty fields" DoD.
        assert_eq!(result.session.status, SessionStatus::Active);
        assert!(result.session.final_package.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn revision_request_reverts_phase_and_logs_entry() {
        let text = envelope_text(
            "Entendido, volvamos a tu disponibilidad horaria.",
            "syllabus",
            "{}",
            "{}",
            "ask_question",
        )
        .replace(r#""revision_requested":null"#, r#""revision_requested":"exploration""#);
        let (service, dir) = service_with(vec![&text]);
        let result = service.start_session(None).await.expect("turn ok");

        assert_eq!(result.session.phase, RoadmapPhase::Exploration);
        assert_eq!(result.session.revision_log.len(), 1);
        assert_eq!(result.session.revision_log[0].reverted_to_phase, RoadmapPhase::Exploration);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn sealing_succeeds_with_a_fully_populated_negotiation_bucket() {
        let dod = r#"{"aprobacion_formal_recibida": true, "payload_sin_campos_vacios": true, "contexto_autosuficiente_para_notebook_builder": true}"#;
        let patch = r#"{"executive_summary": "Aprenderás Rust construyendo un CLI.", "approval": {"approved_verbatim_quote": "Sí, apruebo el plan", "approved_at_turn": 5}, "handoff_notes_for_notebook_builder": "Prioriza ejemplos con async/await."}"#;
        let text = envelope_text("¡Excelente! Tu roadmap queda sellado.", "negotiation", dod, patch, "final_delivery");
        let (service, dir) = service_with(vec![&text]);
        let mut session = RoadmapSession::new("SID".to_string(), 0);
        session.phase = RoadmapPhase::Negotiation;
        session.learner_profile = serde_json::json!({
            "declared_topic": "Rust", "time_horizon_weeks": 8, "hours_per_week": 6.0,
            "self_perceived_level": "novice", "level_justification": "nunca programó en Rust"
        });
        session.diagnostic_summary = serde_json::json!({
            "critical_gaps": [], "capstone_options_presented": ["CLI de tareas"],
            "capstone_selected": "CLI de tareas", "difficulty_calibration_note": "ajustado a nivel novato"
        });
        session.syllabus = serde_json::json!({
            "milestones": [{
                "id": "m1", "title": "Fundamentos", "bloom_level": "apply",
                "learning_objective": "Aplicar ownership en un programa pequeño",
                "authentic_challenge": "Construir un parser de argumentos",
                "notebook_block_plan": ["micro_simulator"], "estimated_hours": 4.0
            }]
        });
        service.store.save_roadmap_session(&session).expect("seed session");

        let result = service.send_message(None, "SID", "Sí, apruebo el plan").await.expect("turn ok");

        assert_eq!(result.session.status, SessionStatus::Sealed);
        let package = result.session.final_package.expect("package present");
        assert_eq!(package.session_id, "SID");
        assert_eq!(package.approval.approved_verbatim_quote, "Sí, apruebo el plan");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
