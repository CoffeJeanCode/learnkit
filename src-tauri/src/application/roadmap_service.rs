use std::sync::Arc;

use uuid::Uuid;

use crate::domain::notebook::{DiagnosticBattery, DiagnosticBatteryState, DiagnosticDimension, DiagnosticQuestion};
use crate::domain::roadmap::{
    CapstoneProject, ChatTurn, DiagnosticAssessmentArgs, DiagnosticSummaryCard, EntryLevel, LearnerProfileCard, Micromodule,
    Milestone, ProposedPlan, RoadmapPhase, RoadmapSession, RoadmapSessionSummary, RoadmapSyllabusPackage, SealedRoadmap,
    SessionStatus,
};
use crate::error::{AppError, AppResult};
use crate::notebook_store::NotebookStore;
use crate::orchestration::Orchestrator;
use crate::persistence::FileStore;

#[derive(Debug)]
pub struct RoadmapTurnResult {
    pub message: String,
    pub session: RoadmapSession,
}

/// Welcome variants served locally by `start_session` — no model call (see
/// above). All of them do the same job (warm welcome + ask for their goal)
/// in the mentor's voice and without a word of meta-language, so the student
/// can't tell whether the greeting was written or generated.
const ONBOARDING_MESSAGES: &[&str] = &[
    "¡Hola! Qué bueno verte por aquí. Soy tu mentor y voy a acompañarte a convertir tu meta en un plan que sí puedas terminar. Para empezar: ¿qué quieres lograr y en cuánto tiempo?",
    "¡Bienvenido! Mi trabajo es ayudarte a pasar de la idea a un plan concreto, semana a semana, con tu proyecto como protagonista. Cuéntame: ¿qué tienes entre manos?",
    "¡Hola! Aquí vamos a tu ritmo: sin exámenes, sin prisa. Solo una conversación para entender tu meta y diseñar cómo llegar a ella. ¿Por dónde empezamos?",
    "¡Qué bueno que llegaste! Piensa en mí como tu compañero de ruta: te hago preguntas, celebramos avances y al final tendrás un plan hecho a tu medida. ¿Cuál es tu meta?",
    "¡Hola! Vamos a armar juntos tu ruta de aprendizaje. Primero lo primero: ¿qué te gustaría construir, aprobar o dominar?",
];

/// Deterministic per session (stable across restarts, varied across
/// sessions) without pulling in an RNG crate: FNV-style hash of the
/// session id modulo the variant count.
fn pick_onboarding_message(session_id: &str) -> &'static str {
    let mut hash: u64 = 0;
    for b in session_id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u64);
    }
    ONBOARDING_MESSAGES[(hash as usize) % ONBOARDING_MESSAGES.len()]
}

/// Application service for the Roadmap & Syllabus Diagnostic Agent — an
/// autonomous, tool-calling orchestrator (see `agents::roadmap_agent`) that
/// still avoids confirmation steps EXCEPT the one place they're genuinely
/// required: confirming the plan before it's persisted. A Strict Gated Flow,
/// one agent, 4 tools:
///
/// 1. **Capture** (Gate 1) — `submit_diagnostic_assessment` the instant
///    topic, goal, timeframe and weekly hours are known, AND the student has
///    explicitly self-declared their entry level (one of 3 closed options —
///    never inferred). No pedagogical inference happens here.
/// 2. **Situational battery** (Gate 2) — `present_diagnostic_battery`,
///    chained in the SAME turn as the assessment, UNLESS the declared level
///    is `absolute_zero` (then this gate is skipped entirely). The session
///    then genuinely WAITS: the student answers via [`Self::
///    answer_diagnostic_question`] (or skips via [`Self::
///    skip_diagnostic_battery`]) — no model call, no failure mode, just
///    deterministic grading against `correctOption`.
/// 3. **Propose** (Gate 3a) — the instant Gate 2 finishes (last answer,
///    skip, or the `absolute_zero` bypass), `propose_syllabus_plan` runs
///    automatically: the diagnostic consolidation + syllabus, calibrated by
///    the CONCRETE results instead of the vague self-reported level alone.
///    Nothing is persisted yet — it's a proposal, shown to the student with
///    a closing question.
/// 4. **Confirm** (Gate 3b) — the student's free-text reply flows through a
///    normal chat turn. Confirming calls `confirm_syllabus_plan` (no
///    payload — the syllabus is already agreed on), which seals the session
///    and persists the course. No notebook content is generated at this
///    gate: every class, including the first, gets its notebook lazily via
///    `notebook_service::generation::start_class_notebook` the first time
///    the student actually opens it. Asking for changes calls
///    `propose_syllabus_plan` again with a revision — no hard cap on this
///    negotiation, it's genuine user-driven interaction, not a retry loop.
///
/// The turn cap ([`MAX_GATHER_TURNS`](Self::MAX_GATHER_TURNS)) is the
/// load-bearing guarantee for Gates 1-3, not the prompt: before ever calling
/// the model, [`Self::advance`] checks `session.turn_count_in_phase`. Once
/// that's reached, the model is not called again — the session force-closes
/// from whatever was gathered, with honest fallback defaults for the rest
/// (including a synthesized, unanswered battery, and skipping straight past
/// propose/confirm to a direct seal). Gate 3b needs no such cap: the
/// student's own reply is what drives it, not a retrying model.
pub struct RoadmapService {
    orchestrator: Arc<Orchestrator>,
    store: FileStore,
    notebook_store: NotebookStore,
}

impl RoadmapService {
    /// Turns spent WITHOUT reaching a stable state (sealed, or a battery
    /// successfully generated and waiting on the student) before
    /// force-closing — see [`Self::apply_capture`]'s `stable` check. Covers
    /// both failure shapes: the model never completing
    /// `submit_diagnostic_assessment`, or completing it but never producing
    /// a valid `present_diagnostic_battery`. Once a battery IS successfully
    /// generated, this stops incrementing entirely — the rest of the wait is
    /// on the student, not the model, so there's no more retry-loop risk to
    /// cap.
    const MAX_GATHER_TURNS: u32 = 2;
    /// Transient-failure retry budget for one turn's model call (network
    /// blip, timeout) — there's nothing to correct, the model never
    /// produced output, so just retry the same input. 3, not 2: the
    /// failures actually observed against DeepSeek are near-instant
    /// connection errors (`HttpError: error sending request`, not a slow
    /// timeout), so a single retry sometimes lands in the same brief
    /// outage window — see [`Self::run_diagnostic_turn_with_retry`]'s
    /// backoff between attempts.
    const MAX_EXECUTION_ATTEMPTS: u8 = 3;
    /// In-turn content-grounding retry budget — see
    /// [`Self::run_diagnostic_turn_with_grounding_retry`]. Unlike
    /// `MAX_EXECUTION_ATTEMPTS` (network/transient failures, nothing to
    /// correct), a grounding failure DOES carry something the model can fix
    /// (`last_rejection_reasons`, fed back into the very next attempt), so
    /// it's worth spending an extra attempt on immediately rather than
    /// making the student send another message just to trigger the same
    /// fix. Kept small (2: one real attempt + one correction) — this is
    /// still bounded by `MAX_GATHER_TURNS` on the outside, which only counts
    /// once per user-facing turn regardless of how many attempts this used.
    const MAX_GROUNDING_RETRIES: u32 = 2;
    /// BUG THIS FIXED: `MAX_GATHER_TURNS`/`turn_count_in_phase` freezes
    /// FOREVER the instant a battery is ever generated (by design — Gate 3
    /// is meant to be paced by the student, not capped). But the model can
    /// still get stuck failing grounding indefinitely AFTER that point too
    /// (re-presenting an already-answered battery, or never landing a valid
    /// `propose_syllabus_plan`/`confirm_syllabus_plan`) — and with nothing
    /// watching, that produced a REAL infinite loop: the student kept
    /// getting `run_diagnostic_turn_with_grounding_retry`'s honest fallback
    /// text ("Casi lo tengo…") turn after turn, forever, because nothing
    /// ever forced a resolution. This counter (`RoadmapSession::
    /// consecutive_grounding_failures`) tracks failures ACROSS real turns,
    /// independent of which gate they happen at, and forces progress once
    /// exhausted — see `Self::force_progress_from_gate3` and `advance`'s
    /// check at the top.
    const MAX_CONSECUTIVE_GROUNDING_FAILURES: u32 = 2;
    /// Base delay before a retry, scaled by attempt number (500ms, 1000ms,
    /// …) — long enough to usually clear a brief connection blip, short
    /// enough not to make the student stare at "Pensando…" for too long.
    const RETRY_BACKOFF_BASE: std::time::Duration = std::time::Duration::from_millis(500);

    pub fn new(orchestrator: Arc<Orchestrator>, store: FileStore, notebook_store: NotebookStore) -> Self {
        Self { orchestrator, store, notebook_store }
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<RoadmapSession> {
        self.store
            .load_roadmap_session(session_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("roadmap session not found: {session_id}")))
    }

    /// Saved sessions for the drawer, newest first (summaries only — the
    /// full session loads on open).
    pub fn list_sessions(&self) -> AppResult<Vec<RoadmapSessionSummary>> {
        Ok(self.store.list_roadmap_sessions()?.iter().map(RoadmapSession::summary).collect())
    }

    /// Rename a session's list label. Metadata only: content, phase and
    /// timestamps are untouched (rename must not reorder the drawer).
    pub fn rename_session(&self, session_id: &str, title: &str) -> AppResult<RoadmapSessionSummary> {
        const MAX_TITLE_CHARS: usize = 80;
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::InvalidInput("title is empty".to_string()));
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(AppError::InvalidInput(format!("title too long (max {MAX_TITLE_CHARS} chars)")));
        }
        let mut session = self.get_session(session_id)?;
        session.custom_title = Some(title.to_string());
        self.store.save_roadmap_session(&session)?;
        Ok(session.summary())
    }

    /// Delete a saved session — AND the course it owns. Existence is checked
    /// first so a missing id fails typed instead of silently succeeding.
    ///
    /// A sealed session is the owner of exactly one course (see
    /// `RoadmapSession::imported_course_id`), which in turn owns its
    /// milestones/classes/notebooks by FK cascade. Deleting only the session
    /// JSON (the old behavior) left those courses orphaned in SQLite — they
    /// kept showing up under the "Clases" section with no session behind
    /// them. The session JSON goes first: if the course delete somehow fails
    /// afterwards, the session is already gone, so there's no half-state
    /// where reopening the session resurrects a course the student meant to
    /// destroy (and the startup sweep would remove it on the next launch).
    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        let session = self.get_session(session_id)?;
        self.store.delete_roadmap_session(session_id)?;
        if let Some(course_id) = session.imported_course_id {
            self.notebook_store.delete_course(&course_id)?;
        }
        Ok(())
    }

    /// Startup sweep: removes every course no saved session references —
    /// the cleanup for courses orphaned by sessions deleted BEFORE
    /// [`Self::delete_session`] cascaded (and for any future leak). Idempotent
    /// and cheap (a handful of rows), so it runs on every launch. Returns
    /// the number of courses removed so the caller can log it.
    pub fn sweep_orphan_courses(&self) -> AppResult<usize> {
        let owned: Vec<String> = self
            .store
            .list_roadmap_sessions()?
            .into_iter()
            .filter_map(|s| s.imported_course_id)
            .collect();
        self.notebook_store.delete_courses_except(&owned)
    }

    pub async fn start_session(&self, _app: Option<&tauri::AppHandle>) -> AppResult<RoadmapTurnResult> {
        let mut session = RoadmapSession::new(Uuid::new_v4().to_string(), now_ms());
        // No model call: the greeting always asks the same thing (welcome +
        // "what's your goal"), so spending a 30–120s LLM round-trip on every
        // "Nueva sesión" is pure waste. The model joins on the first real
        // user reply. No gather turn is spent either — the caps only count
        // turns where the model actually ran.
        let message = pick_onboarding_message(&session.session_id).to_string();
        // The greeting opens the persisted log, so reopening the session
        // shows the welcome it was actually given.
        session.messages.push(ChatTurn { role: "assistant".to_string(), text: message.clone(), at_ms: now_ms() });
        self.store.save_roadmap_session(&session)?;
        Ok(RoadmapTurnResult { message, session })
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
        // Blocks free-text turns only while genuinely mid-click-battery
        // (Gate 2's answers come through `answer_diagnostic_question`, not
        // chat). Once the battery is fully answered — or a proposal already
        // exists (Gate 3) — free text is exactly what's expected next
        // (confirm/adjust), so this must NOT block then.
        if let Some(pending) = &session.pending_diagnostic_battery {
            if !pending.fully_answered() {
                return Err(AppError::InvalidInput(
                    "la sesión está esperando las respuestas de la batería de diagnóstico — usa answer_diagnostic_question"
                        .to_string(),
                ));
            }
        }

        // Hard ceiling #1: if every gathering turn has already been spent
        // (Gate 1/2 never completed), do not call the model again —
        // force-close now, in code.
        if session.turn_count_in_phase >= Self::MAX_GATHER_TURNS {
            // The user turn still gets into the log (and persists) even
            // though the model is never called this turn.
            session.messages.push(ChatTurn { role: "user".to_string(), text: user_message.to_string(), at_ms: now_ms() });
            let message = self.force_close_session(&mut session)?;
            session.messages.push(ChatTurn { role: "assistant".to_string(), text: message.clone(), at_ms: now_ms() });
            trim_message_log(&mut session);
            session.updated_at_ms = now_ms();
            self.store.save_roadmap_session(&session)?;
            return Ok(RoadmapTurnResult { message, session });
        }

        // Hard ceiling #2: Gate 1/2 already succeeded (so ceiling #1 above
        // stays frozen forever, by design), but the model has now failed
        // grounding this many REAL turns in a row — re-presenting an
        // already-answered battery, or never landing a valid propose/
        // confirm. Force progress with whatever's real instead of repeating
        // the honest-fallback text yet again.
        if session.consecutive_grounding_failures >= Self::MAX_CONSECUTIVE_GROUNDING_FAILURES {
            session.messages.push(ChatTurn { role: "user".to_string(), text: user_message.to_string(), at_ms: now_ms() });
            let message = self.force_progress_from_gate3(&mut session)?;
            session.messages.push(ChatTurn { role: "assistant".to_string(), text: message.clone(), at_ms: now_ms() });
            trim_message_log(&mut session);
            session.updated_at_ms = now_ms();
            self.store.save_roadmap_session(&session)?;
            return Ok(RoadmapTurnResult { message, session });
        }

        record_message(&mut session, user_message);
        // Persist the user turn BEFORE the model call: a failed turn (key,
        // network) must not lose what the student wrote.
        session.messages.push(ChatTurn { role: "user".to_string(), text: user_message.to_string(), at_ms: now_ms() });
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;

        let (agent_id, scope) = Self::turn_agent_and_scope(&session);
        let (mut text, mut had_violation) = self
            .run_diagnostic_turn_with_grounding_retry(app, &mut session, agent_id, scope, |s| render_turn_input(s, user_message, scope))
            .await?;
        // Follow-up hand-offs (Gate 2 battery, Gate 3a propose) run as their
        // own scoped turns right behind a CLEAN main turn — each replaces the
        // main turn's text, and its own grounding failures feed the same
        // consecutive-failure ceiling. Only ONE can apply per advance (they
        // are sequential stages), and never mid-seal (status re-checked
        // against the post-main-turn session).
        if !had_violation {
            if Self::battery_followup_needed(&session) {
                let (t, v) = self
                    .run_diagnostic_turn_with_grounding_retry(
                        app,
                        &mut session,
                        ROADMAP_AGENT_ID,
                        DiagnosticToolScope::Battery,
                        render_battery_followup_input,
                    )
                    .await?;
                text = t;
                had_violation = v;
            } else if Self::propose_followup_needed(&session) {
                let (t, v) = self
                    .run_diagnostic_turn_with_grounding_retry(
                        app,
                        &mut session,
                        ROADMAP_AGENT_ID,
                        DiagnosticToolScope::Propose,
                        |s| match s.learner_profile_card.as_ref() {
                            Some(profile) => render_propose_syllabus_input(
                                profile,
                                s.pending_diagnostic_battery.as_ref(),
                                &s.last_rejection_reasons,
                            ),
                            None => String::new(),
                        },
                    )
                    .await?;
                text = t;
                had_violation = v;
            }
        }
        session.consecutive_grounding_failures = if had_violation { session.consecutive_grounding_failures + 1 } else { 0 };
        Self::record_gather_turn_if_unstable(&mut session);
        session.messages.push(ChatTurn { role: "assistant".to_string(), text: text.trim().to_string(), at_ms: now_ms() });
        trim_message_log(&mut session);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;

        Ok(RoadmapTurnResult { message: text.trim().to_string(), session })
    }

    /// Records one answer to the pending diagnostic battery. Purely
    /// deterministic (Rust compares against `correctOption` — no model call,
    /// no failure mode to loop on) until the LAST question is answered, at
    /// which point this immediately triggers Gate 3a: one more model turn,
    /// now with the concrete diagnostic results as context, expected to
    /// call ONLY `propose_syllabus_plan`. No extra user action needed for
    /// that hand-off — the only real wait so far is for the student's
    /// actual answers, which just arrived; the NEXT wait (confirming the
    /// proposal) is a normal free-text chat turn, not this method.
    #[tracing::instrument(skip(self, app), fields(session_id = %session_id))]
    pub async fn answer_diagnostic_question(
        &self,
        app: Option<&tauri::AppHandle>,
        session_id: &str,
        question_index: usize,
        answer: &str,
    ) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        let mut pending = session
            .pending_diagnostic_battery
            .clone()
            .ok_or_else(|| AppError::InvalidInput("no hay una batería de diagnóstico pendiente para esta sesión".to_string()))?;
        if question_index >= pending.battery.questions.len() {
            return Err(AppError::InvalidInput(format!("question_index {question_index} fuera de rango")));
        }
        pending.answers.insert(question_index.to_string(), answer.to_string());
        let done = pending.fully_answered();
        session.pending_diagnostic_battery = Some(pending);

        if !done {
            session.updated_at_ms = now_ms();
            self.store.save_roadmap_session(&session)?;
            return Ok(RoadmapTurnResult { message: String::new(), session });
        }

        // Persist the FINAL answer BEFORE the Gate 3a model turn: without
        // this, a network failure inside `finish_diagnostic_stage` lost the
        // student's last click along with the turn (the only save used to be
        // the one after the model replied), leaving a half-answered battery
        // on disk that no retry could reconstruct.
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;
        self.finish_diagnostic_stage(app, session).await
    }

    /// Re-runs the turn that just FAILED after the backend's own retry
    /// budget ([`Self::MAX_EXECUTION_ATTEMPTS`]) was exhausted — the target
    /// of the UI's manual "Reintentar" and of its one automatic re-attempt
    /// (`retry_roadmap_turn` in `commands::roadmap`).
    ///
    /// It picks up exactly where the error left off instead of replaying the
    /// student's input through [`Self::advance`], which would duplicate the
    /// user turn in the log: a failed `advance` already persisted that turn
    /// before calling the model. Two recoverable shapes:
    ///
    /// 1. the log ends with a `user` turn — the send path died mid-model;
    /// 2. the battery is fully answered but no `proposed_plan` ever landed —
    ///    the Gate 3a turn died (including the final-answer hand-off).
    ///
    /// An error here is non-recoverable state (sealed session, nothing
    /// pending), surfaced as `InvalidInput` so the UI drops its retry affordance.
    #[tracing::instrument(skip(self, app), fields(session_id = %session_id))]
    pub async fn retry_pending_turn(
        &self,
        app: Option<&tauri::AppHandle>,
        session_id: &str,
    ) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }

        // Shape 1: a user message with no assistant reply yet — the normal
        // send path. The turn already sits in the log AND in `draft`, so run
        // the model against it without recording anything again.
        if let Some(user_message) = session.messages.last().filter(|m| m.role == "user").map(|m| m.text.clone()) {
            let (agent_id, scope) = Self::turn_agent_and_scope(&session);
            let (text, had_violation) = self
                .run_diagnostic_turn_with_grounding_retry(app, &mut session, agent_id, scope, |s| {
                    render_turn_input(s, &user_message, scope)
                })
                .await?;
            session.consecutive_grounding_failures =
                if had_violation { session.consecutive_grounding_failures + 1 } else { 0 };
            Self::record_gather_turn_if_unstable(&mut session);
            session.messages.push(ChatTurn { role: "assistant".to_string(), text: text.trim().to_string(), at_ms: now_ms() });
            trim_message_log(&mut session);
            session.updated_at_ms = now_ms();
            self.store.save_roadmap_session(&session)?;
            return Ok(RoadmapTurnResult { message: text.trim().to_string(), session });
        }

        // Shape 2: the proposal turn never landed — re-run Gate 3a exactly
        // as the final-answer hand-off would have.
        if session.learner_profile_card.is_some()
            && session.pending_diagnostic_battery.as_ref().map(|p| p.fully_answered()).unwrap_or(false)
            && session.proposed_plan.is_none()
        {
            return self.finish_diagnostic_stage(app, session).await;
        }

        Err(AppError::InvalidInput("no hay un turno pendiente que reintentar".to_string()))
    }

    /// Escape hatch: finishes the Diagnostic stage with whatever's answered
    /// so far (possibly nothing) — never let a student get stuck on the
    /// battery with no way forward, same "never leaves the student stuck"
    /// guarantee the onboarding turn cap already provides.
    #[tracing::instrument(skip(self, app), fields(session_id = %session_id))]
    pub async fn skip_diagnostic_battery(&self, app: Option<&tauri::AppHandle>, session_id: &str) -> AppResult<RoadmapTurnResult> {
        let session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        if session.pending_diagnostic_battery.is_none() {
            return Err(AppError::InvalidInput("no hay una batería de diagnóstico pendiente para esta sesión".to_string()));
        }
        self.finish_diagnostic_stage(app, session).await
    }

}

mod flow;
mod grounding;
mod render;
mod session;
mod synth;

use crate::agents::roadmap_agent::ROADMAP_AGENT_ID;
use crate::tools::DiagnosticToolScope;
use render::{render_battery_followup_input, render_propose_syllabus_input, render_turn_input};
use session::{now_ms, record_message, trim_message_log};

#[cfg(test)]
mod tests;
