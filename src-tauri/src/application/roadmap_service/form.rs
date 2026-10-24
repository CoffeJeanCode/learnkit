//! Form-driven entry points of [`RoadmapService`]: the structured replacement
//! for the free-chat turns. The learner fills a profile form, confirms with a
//! button and adjusts a proposal with controls — so the analyst turns and the
//! "Sí, me parece bien." confirmation turn (a model call spent only to emit a
//! payload-less tool) no longer exist. Everything downstream — grounding,
//! the battery, the proposal, `seal_session` — is the SAME code the chat path
//! drives, so a form session is indistinguishable from a chat one once saved.

use super::grounding::*;
use super::render::*;
use super::session::*;
use super::*;
use std::sync::Mutex;

use crate::agents::context_questions_agent::CONTEXT_QUESTIONS_AGENT_ID;
use crate::domain::learning_context::{
    ContextAnswer, ContextQuestionSet, LearningContext, derive_entry_level, fallback_question_set, validate_question_set,
};
use crate::domain::roadmap::PlanEmphasis;
use crate::tools::{ResearchCapture, ResearchStep, SharedResearchCapture};

/// Shown when the hand-off behind the form (the battery, or the proposal for
/// `absolute_zero`) could not be produced after the in-turn retries. The form
/// flow has no composer to "write again" in, so it points at the retry button.
const HAND_OFF_FAILED_TEXT: &str = "No pude armar el siguiente paso todavía. Pulsa «Reintentar» para volver a intentarlo.";

const SEALED_TEXT: &str = "¡Listo! Tu plan quedó guardado — tu primera clase se genera en cuanto la abras.";

const ADJUST_FAILED_TEXT: &str = "No pude aplicar ese ajuste, así que conservé tu propuesta anterior. Prueba con otros valores o inténtalo de nuevo.";

/// Attempts at getting a valid question set from the model before falling back.
const MAX_QUESTION_ATTEMPTS: u8 = 2;

/// The one-line user turn a profile submit leaves in the visible log, so the
/// history still reads as a conversation. The level is NOT in it: it was
/// deduced from the answers, not something the learner said.
fn profile_summary_text(card: &LearnerProfileCard) -> String {
    format!("Tema: {} · Meta: {} · {} semanas · {} h/semana", card.topic, card.target_goal, card.timeframe_weeks, card.weekly_commitment_hours)
}

fn adjustment_summary_text(adj: &PlanAdjustment) -> String {
    let mut parts = vec!["Quiero ajustar el plan".to_string()];
    if let Some(w) = adj.timeframe_weeks {
        parts.push(format!("{w} semanas"));
    }
    if let Some(h) = adj.weekly_commitment_hours {
        parts.push(format!("{h} h/semana"));
    }
    if let Some(e) = adj.emphasis {
        parts.push(match e {
            PlanEmphasis::MorePractice => "más práctica",
            PlanEmphasis::MoreTheory => "más teoría",
            PlanEmphasis::MoreProjects => "más proyecto",
        }
        .to_string());
    }
    if let Some(n) = adj.clean_note() {
        parts.push(format!("«{n}»"));
    }
    parts.join(" · ")
}

impl RoadmapService {
    /// Step 1 -> step 2 of the profile form. Validates the four facts the learner
    /// typed (topic, goal, weeks, hours) with the same ranges as always, asks the
    /// model for intake questions SPECIFIC to this topic, validates them in code
    /// and keeps them on the session as the server's own copy. If the model
    /// cannot produce a valid set, a generic one is used: the form never
    /// dead-ends. Costs one fast model call; nothing is spent on invalid input.
    #[tracing::instrument(skip(self, app, basics), fields(session_id = %session_id))]
    pub async fn propose_context_questions(&self, app: Option<&tauri::AppHandle>, session_id: &str, basics: ProfileBasics) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        if session.learner_profile_card.is_some() {
            return Err(AppError::InvalidInput("el perfil ya fue registrado en esta sesión".to_string()));
        }
        let basics = basics.cleaned();
        // Same grounding as the saved profile; the level is a placeholder here
        // (it is deduced at step 2 and plays no part in these checks).
        let probe = build_profile_card(&DiagnosticAssessmentArgs {
            topic: basics.topic.clone(),
            target_goal: basics.target_goal.clone(),
            entry_level: EntryLevel::TheoreticalFoundations,
            timeframe_weeks: basics.timeframe_weeks,
            weekly_commitment_hours: basics.weekly_commitment_hours,
        });
        let violations = profile_grounding_violations(&probe);
        if !violations.is_empty() {
            return Err(AppError::InvalidInput(violations.join("; ")));
        }

        let questions = self.generate_question_set(app, &basics).await;
        session.profile_basics = Some(basics);
        session.pending_context_questions = Some(questions);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;
        Ok(RoadmapTurnResult { message: String::new(), session })
    }

    /// The model proposes, the code decides: up to [`MAX_QUESTION_ATTEMPTS`]
    /// tries, each retry told why the last was refused; then the generic set.
    async fn generate_question_set(&self, app: Option<&tauri::AppHandle>, basics: &ProfileBasics) -> ContextQuestionSet {
        let mut feedback: Vec<String> = Vec::new();
        for attempt in 0..MAX_QUESTION_ATTEMPTS {
            let input = render_context_questions_input(basics, &feedback);
            let capture: SharedResearchCapture = Arc::new(Mutex::new(ResearchCapture::default()));
            match self.orchestrator.run_research_agent(app, CONTEXT_QUESTIONS_AGENT_ID, &input, capture.clone(), ResearchStep::Questions).await {
                Ok(_) => {
                    let proposed = capture.lock().unwrap_or_else(|e| e.into_inner()).questions.clone();
                    match proposed.map(|args| validate_question_set(&args)) {
                        Some(Ok(set)) => return set,
                        Some(Err(reasons)) => {
                            tracing::warn!(attempt, ?reasons, "intake questions failed validation");
                            crate::dev_trace::trace_note("critique", "context_questions", &basics.topic, "rejected", &reasons);
                            feedback = reasons;
                        }
                        None => feedback = vec!["No llamaste a submit_context_questions: llámala ahora.".to_string()],
                    }
                }
                Err(e) if e.is_transient() => tracing::warn!(attempt, error = %e, "intake questions call failed, retrying"),
                Err(e) => {
                    tracing::warn!(error = %e, "intake questions unavailable");
                    break;
                }
            }
        }
        tracing::warn!(topic_chars = basics.topic.chars().count(), "using the generic intake questions");
        fallback_question_set(&basics.topic)
    }

    /// Step 2: validates the answers against the questions the SERVER kept (never
    /// against what the client says they were), deduces the starting level from
    /// the prior-knowledge answers, saves the profile and runs the hand-off behind
    /// it (the battery, or the proposal when the level is `absolute_zero`).
    /// Invalid answers come back as `InvalidInput` with every reason.
    #[tracing::instrument(skip(self, app, answers), fields(session_id = %session_id))]
    pub async fn submit_profile(&self, app: Option<&tauri::AppHandle>, session_id: &str, answers: Vec<ContextAnswer>) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        if session.learner_profile_card.is_some() {
            return Err(AppError::InvalidInput("el perfil ya fue registrado en esta sesión".to_string()));
        }
        let (Some(basics), Some(questions)) = (session.profile_basics.clone(), session.pending_context_questions.clone()) else {
            return Err(AppError::InvalidInput("primero completa el tema, la meta y el tiempo".to_string()));
        };
        let context = LearningContext::validate(&questions, &answers).map_err(|e| AppError::InvalidInput(e.join("; ")))?;
        let args = DiagnosticAssessmentArgs {
            topic: basics.topic,
            target_goal: basics.target_goal,
            entry_level: derive_entry_level(&questions, &context),
            timeframe_weeks: basics.timeframe_weeks,
            weekly_commitment_hours: basics.weekly_commitment_hours,
        };
        let card = build_profile_card(&args);
        let violations = profile_grounding_violations(&card);
        if !violations.is_empty() {
            return Err(AppError::InvalidInput(violations.join("; ")));
        }

        session.messages.push(ChatTurn { role: "user".to_string(), text: profile_summary_text(&card), at_ms: now_ms() });
        session.learner_profile_card = Some(card);
        session.learning_context = Some(context);
        session.profile_basics = None;
        session.pending_context_questions = None;
        if session.phase == RoadmapPhase::Onboarding {
            session.phase = RoadmapPhase::Diagnostic;
        }
        session.last_rejection_reasons.clear();
        // Durable BEFORE any model call: if the hand-off below dies, the
        // profile is already on disk and `retry_pending_turn` resumes from it.
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;
        self.finish_profile_hand_off(app, session).await
    }

    /// Runs the hand-off behind a saved profile and records its outcome —
    /// shared by [`Self::submit_profile`] and `retry_pending_turn`.
    pub(super) async fn finish_profile_hand_off(&self, app: Option<&tauri::AppHandle>, mut session: RoadmapSession) -> AppResult<RoadmapTurnResult> {
        let (mut text, had_violation) = self.run_followup_hand_off(app, &mut session).await?.unwrap_or_default();
        session.consecutive_grounding_failures = if had_violation { session.consecutive_grounding_failures + 1 } else { 0 };
        Self::record_gather_turn_if_unstable(&mut session);
        if had_violation {
            text = HAND_OFF_FAILED_TEXT.to_string();
        }
        let text = text.trim().to_string();
        if !text.is_empty() {
            session.messages.push(ChatTurn { role: "assistant".to_string(), text: text.clone(), at_ms: now_ms() });
        }
        trim_message_log(&mut session);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;
        Ok(RoadmapTurnResult { message: text, session })
    }

    /// Seals the proposed plan WITHOUT a model call. The chat path spent a
    /// whole turn (scope `All`) only so the model would call
    /// `confirm_syllabus_plan`, a tool with no payload; here the button itself
    /// is the confirmation. Same `seal_session` as every other path.
    #[tracing::instrument(skip(self), fields(session_id = %session_id))]
    pub fn confirm_proposed_plan(&self, session_id: &str) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        let (profile, plan) = match (session.learner_profile_card.clone(), session.proposed_plan.clone()) {
            (Some(profile), Some(plan)) => (profile, plan),
            _ => return Err(AppError::InvalidInput("no hay un plan propuesto para confirmar".to_string())),
        };
        let battery = session.pending_diagnostic_battery.clone();
        self.seal_session(&mut session, &profile, plan.syllabus, battery)?;
        session.messages.push(ChatTurn { role: "user".to_string(), text: "Confirmo el plan.".to_string(), at_ms: now_ms() });
        session.messages.push(ChatTurn { role: "assistant".to_string(), text: SEALED_TEXT.to_string(), at_ms: now_ms() });
        trim_message_log(&mut session);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;
        Ok(RoadmapTurnResult { message: SEALED_TEXT.to_string(), session })
    }

    /// Revises a PROPOSED plan from structured controls (weeks, hours,
    /// emphasis, an optional short note) — one proposal turn, no negotiation
    /// chat. Weeks/hours update the profile card first, so the new proposal is
    /// grounded against what the learner NOW declares.
    ///
    /// Never leaves the learner without a plan: if the revised proposal cannot
    /// be produced (transport error or grounding failure) the previous profile
    /// and proposal are restored and the previous plan stays on screen.
    #[tracing::instrument(skip(self, app, adjustment), fields(session_id = %session_id))]
    pub async fn adjust_proposed_plan(&self, app: Option<&tauri::AppHandle>, session_id: &str, adjustment: PlanAdjustment) -> AppResult<RoadmapTurnResult> {
        let mut session = self.get_session(session_id)?;
        if session.status == SessionStatus::Sealed {
            return Err(AppError::InvalidInput("roadmap session already sealed".to_string()));
        }
        if adjustment.is_empty() {
            return Err(AppError::InvalidInput("indica al menos un cambio para ajustar el plan".to_string()));
        }
        let (previous_profile, previous_plan) = match (session.learner_profile_card.clone(), session.proposed_plan.clone()) {
            (Some(profile), Some(plan)) => (profile, plan),
            _ => return Err(AppError::InvalidInput("no hay un plan propuesto para ajustar".to_string())),
        };

        let mut profile = previous_profile.clone();
        if let Some(weeks) = adjustment.timeframe_weeks {
            profile.timeframe_weeks = weeks;
        }
        if let Some(hours) = adjustment.weekly_commitment_hours {
            profile.weekly_commitment_hours = hours;
        }
        profile.total_available_hours = profile.timeframe_weeks as f32 * profile.weekly_commitment_hours;
        let violations = profile_grounding_violations(&profile);
        if !violations.is_empty() {
            return Err(AppError::InvalidInput(violations.join("; ")));
        }

        session.messages.push(ChatTurn { role: "user".to_string(), text: adjustment_summary_text(&adjustment), at_ms: now_ms() });
        session.learner_profile_card = Some(profile);
        session.proposed_plan = None;
        session.last_rejection_reasons.clear();
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;

        let outcome = self.run_propose_turn(app, &mut session, Some((&previous_plan, &adjustment))).await;

        let restore = |session: &mut RoadmapSession| {
            session.learner_profile_card = Some(previous_profile.clone());
            session.proposed_plan = Some(previous_plan.clone());
            session.last_rejection_reasons.clear();
        };
        match outcome {
            Ok((text, false)) if session.proposed_plan.is_some() => {
                session.consecutive_grounding_failures = 0;
                let text = text.trim().to_string();
                session.messages.push(ChatTurn { role: "assistant".to_string(), text: text.clone(), at_ms: now_ms() });
                trim_message_log(&mut session);
                session.updated_at_ms = now_ms();
                self.store.save_roadmap_session(&session)?;
                Ok(RoadmapTurnResult { message: text, session })
            }
            Ok(_) => {
                restore(&mut session);
                session.messages.push(ChatTurn { role: "assistant".to_string(), text: ADJUST_FAILED_TEXT.to_string(), at_ms: now_ms() });
                trim_message_log(&mut session);
                session.updated_at_ms = now_ms();
                self.store.save_roadmap_session(&session)?;
                Ok(RoadmapTurnResult { message: ADJUST_FAILED_TEXT.to_string(), session })
            }
            Err(e) => {
                // The model never answered (key, network): put the previous
                // plan back on disk so a reopen does not show a half-edit.
                restore(&mut session);
                session.updated_at_ms = now_ms();
                let _ = self.store.save_roadmap_session(&session);
                Err(e)
            }
        }
    }
}
