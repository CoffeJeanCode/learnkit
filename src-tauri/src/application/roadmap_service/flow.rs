//! Turn-processing half of [`RoadmapService`]: finishing the diagnostic
//! stage, the grounding-retry loop, capture application, sealing and the
//! force-progress fallbacks. An inherent impl split out of the parent file
//! purely to keep files readable â€” same module tree, same privacy scope.

use std::sync::Mutex;

use crate::agents::analyst_agent::ANALYST_AGENT_ID;
use crate::agents::roadmap_agent::ROADMAP_AGENT_ID;
use crate::tools::{DiagnosticToolScope, RoadmapCapture, SharedRoadmapCapture};

use super::*;
use super::grounding::*;
use super::render::*;
use super::session::*;
use super::synth::*;

impl RoadmapService {
    /// Shared tail of both paths above: runs the Gate-3a model turn
    /// (propose the diagnostic consolidation + syllabus, informed by the
    /// concrete battery results) and applies its capture exactly like a
    /// normal chat turn would. Logs the proposal text into the chat history
    /// like any other assistant turn.
    pub(super) async fn finish_diagnostic_stage(&self, app: Option<&tauri::AppHandle>, mut session: RoadmapSession) -> AppResult<RoadmapTurnResult> {
        let profile = session
            .learner_profile_card
            .clone()
            .ok_or_else(|| AppError::InvalidInput("session has no diagnostic assessment yet".to_string()))?;
        let pending = session.pending_diagnostic_battery.clone().expect("checked by both callers");
        let (text, had_violation) = self
            .run_diagnostic_turn_with_grounding_retry(
                app,
                &mut session,
                ROADMAP_AGENT_ID,
                DiagnosticToolScope::Propose,
                |s| render_propose_syllabus_input(&profile, Some(&pending), &s.last_rejection_reasons),
            )
            .await?;
        session.consecutive_grounding_failures = if had_violation { session.consecutive_grounding_failures + 1 } else { 0 };
        Self::record_gather_turn_if_unstable(&mut session);
        let text = text.trim().to_string();
        session.messages.push(ChatTurn { role: "assistant".to_string(), text: text.clone(), at_ms: now_ms() });
        trim_message_log(&mut session);
        session.updated_at_ms = now_ms();
        self.store.save_roadmap_session(&session)?;

        Ok(RoadmapTurnResult { message: text, session })
    }

    /// Runs one turn against the tool-enabled agent, retrying transient
    /// execution failures only — there is no envelope to parse or correct
    /// anymore, tool args arrive already structured, so the old "malformed
    /// JSON" retry class no longer exists.
    pub(super) async fn run_diagnostic_turn_with_retry(
        &self,
        app: Option<&tauri::AppHandle>,
        agent_id: &str,
        scope: DiagnosticToolScope,
        input: &str,
    ) -> AppResult<(String, RoadmapCapture)> {
        let mut last_err = None;
        for attempt in 0..Self::MAX_EXECUTION_ATTEMPTS {
            let capture: SharedRoadmapCapture = Arc::new(Mutex::new(RoadmapCapture::default()));
            match self.orchestrator.run_diagnostic_agent(app, agent_id, input, capture.clone(), scope).await {
                Ok(out) => {
                    let captured = capture.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    return Ok((out.text, captured));
                }
                Err(e) if e.is_transient() && attempt + 1 < Self::MAX_EXECUTION_ATTEMPTS => {
                    let backoff = Self::RETRY_BACKOFF_BASE * (attempt as u32 + 1);
                    tracing::warn!(error = %e, attempt, backoff_ms = backoff.as_millis(), "roadmap diagnostic turn failed, retrying after backoff");
                    last_err = Some(e);
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.expect("loop only exits None via an Err branch above"))
    }

    /// Applies whatever the turn's tool calls produced — shared by every
    /// caller that runs a roadmap-agent turn (`advance` for onboarding/
    /// confirm chat, `finish_diagnostic_stage` for the propose turn). Gate 1
    /// (`assessment`) and Gate 2 (`diagnostic_battery`) are expected to
    /// chain in the SAME turn — UNLESS `entryLevel` is `AbsoluteZero`, in
    /// which case `assessment` chains with `propose_syllabus_plan` directly
    /// instead (Gate 2 skipped). `propose_syllabus_plan` itself always
    /// expects `propose_capstone_project` in that SAME turn too (split apart
    /// purely to keep each completion small — see `tools::roadmap_tools::
    /// ProposeCapstoneProjectTool`); missing one without the other is treated
    /// as a grounding failure below, not a silent pass-through.
    /// `confirm_syllabus_plan` is never expected alongside any of the above —
    /// it only arrives once a proposal exists.
    ///
    /// BUG FIXED HERE (kept from the earlier one-shot design): this used to
    /// gate `turn_count_in_phase` on a local `acted` flag that went true the
    /// moment `submit_diagnostic_assessment` alone succeeded — even if the
    /// SAME turn's other tool call failed grounding right after it, letting
    /// a persistently-failing call hide behind a repeatedly-resent valid
    /// assessment and loop forever. Gate on whether the session reached a
    /// STABLE checkpoint instead (sealed, a battery now waiting on the
    /// student, or a plan now waiting on the student's confirmation) — the
    /// only states that mean real progress happened and a model-side retry
    /// loop can't recur. See [`record_gather_turn_if_unstable`] — that
    /// increment now lives there, not here (see the return value below).
    ///
    /// Returns `true` if ANY tool call this turn was rejected on content
    /// grounding — the caller uses this to decide whether to immediately
    /// retry the model within the SAME user-facing turn (see
    /// [`Self::run_diagnostic_turn_with_grounding_retry`]), instead of
    /// leaving the student staring at a reply that promises a diagnostic/
    /// plan that never actually renders until they happen to send another
    /// message.
    pub(super) fn apply_capture(&self, session: &mut RoadmapSession, capture: RoadmapCapture) -> AppResult<bool> {
        let mut had_violation = false;

        if let Some(assessment) = capture.assessment {
            let card = build_profile_card(&assessment);
            let violations = profile_grounding_violations(&card);
            if violations.is_empty() {
                session.learner_profile_card = Some(card);
                if session.phase == RoadmapPhase::Onboarding {
                    session.phase = RoadmapPhase::Diagnostic;
                }
                session.last_rejection_reasons.clear();
            } else {
                tracing::warn!(session_id = %session.session_id, ?violations, "submit_diagnostic_assessment failed grounding");
                session.last_rejection_reasons = violations;
                had_violation = true;
            }
        }

        if let Some(battery) = capture.diagnostic_battery {
            match &session.pending_diagnostic_battery {
                Some(pending) if !pending.fully_answered() => {
                    // Already have one, still being answered — a redundant
                    // resend (e.g. the model re-chaining it alongside an
                    // unrelated retry in the SAME turn) must never wipe the
                    // student's progress.
                }
                Some(_) => {
                    // BUG THIS FIXED: the battery is already fully answered
                    // (Gate 2 is done — Gate 3 may even be past propose
                    // already), but the model called this tool AGAIN. Used
                    // to be silently absorbed as a harmless no-op — but its
                    // accompanying TEXT ("Antes de armar tu plan, quiero ver
                    // qué tanto conectas ya con esto:") still described a
                    // battery the student would never see rendered again
                    // (Gate 2 is over), producing an infinite "aquí está tu
                    // batería" loop with nothing behind it every time an
                    // ambiguous reply ("no veo nada") confused the model.
                    // Flag it as a real violation so the grounding-retry
                    // corrects course (or the honest fallback text kicks in)
                    // instead of repeating a promise that never delivers.
                    tracing::warn!(session_id = %session.session_id, "present_diagnostic_battery called again after the battery was already answered");
                    session.last_rejection_reasons = vec![
                        "present_diagnostic_battery se llamó de nuevo, pero la batería ya fue respondida por completo — \
                         la Compuerta 2 ya terminó. NO la vuelvas a presentar ni te refieras a ella como si fuera nueva. \
                         Continúa según el estado actual: propose_syllabus_plan si todavía no hay un plan propuesto, o \
                         interpreta la respuesta del estudiante frente al plan ya propuesto (confirm_syllabus_plan si \
                         confirma, propose_syllabus_plan de nuevo si pide cambios)."
                            .to_string(),
                    ];
                    had_violation = true;
                }
                None if session.learner_profile_card.is_none() => {
                    tracing::warn!(session_id = %session.session_id, "present_diagnostic_battery called before an assessment");
                    session.last_rejection_reasons =
                        vec!["present_diagnostic_battery se llamó antes de submit_diagnostic_assessment".to_string()];
                    had_violation = true;
                }
                None => {
                    let violations = diagnostic_battery_violations(&battery);
                    if violations.is_empty() {
                        session.pending_diagnostic_battery = Some(DiagnosticBatteryState { battery, answers: Default::default() });
                        session.last_rejection_reasons.clear();
                    } else {
                        tracing::warn!(session_id = %session.session_id, ?violations, "present_diagnostic_battery failed grounding");
                        session.last_rejection_reasons = violations;
                        had_violation = true;
                    }
                }
            }
        }

        if let Some(mut propose) = capture.propose_syllabus_plan {
            let capstone = capture.propose_capstone_project;
            match session.learner_profile_card.clone() {
                Some(profile) if battery_satisfied(&profile, &session.pending_diagnostic_battery) => {
                    match capstone {
                        Some(capstone) => {
                            // `paceHoursPerWeek` is never trusted from the model: it's
                            // arithmetically determined by `weeklyCommitmentHours`
                            // (Gate 1, already grounded) — same class of bug
                            // `build_profile_card`'s `total_available_hours` comment
                            // describes ("recurring bug when it was a model-supplied
                            // value"). Overwriting it here means the model only has
                            // ONE number to get right per week (the micromodule-hours
                            // sum), not two independent ones that must agree with
                            // each other AND with the diagnostic.
                            propose.syllabus.pace_hours_per_week = profile.weekly_commitment_hours;
                            // `capstoneProject` no longer arrives on `propose`'s own
                            // JSON payload (see `tools::roadmap_tools::
                            // syllabus_json_schema` — it was split out into its own
                            // `propose_capstone_project` tool call to keep this
                            // completion small enough for DeepSeek's output-token
                            // cap). Merge the separately-captured value in HERE,
                            // before grounding runs, so `syllabus_violations`'s
                            // `capstone_project_violations` check sees the real
                            // thing exactly like it always has.
                            propose.syllabus.capstone_project = capstone.capstone_project;
                            let mut violations = syllabus_violations(&profile, &propose.syllabus);
                            if propose.core_focus.trim().is_empty() {
                                violations.push("coreFocus está vacío".to_string());
                            }
                            if propose.identified_needs.is_empty() {
                                violations.push("identifiedNeeds está vacío".to_string());
                            }
                            if propose.learning_strategy.trim().is_empty() {
                                violations.push("learningStrategy está vacío".to_string());
                            }
                            if propose.closing_question.trim().is_empty() {
                                violations.push("closingQuestion está vacío".to_string());
                            }
                            if violations.is_empty() {
                                let diagnostic_summary = DiagnosticSummaryCard {
                                    core_focus: propose.core_focus,
                                    identified_needs: propose.identified_needs,
                                    learning_strategy: propose.learning_strategy,
                                };
                                session.diagnostic_summary_card = Some(diagnostic_summary.clone());
                                session.proposed_plan = Some(ProposedPlan {
                                    diagnostic_summary,
                                    syllabus: propose.syllabus,
                                    closing_question: propose.closing_question,
                                });
                                session.last_rejection_reasons.clear();
                            } else {
                                tracing::warn!(session_id = %session.session_id, ?violations, "propose_syllabus_plan failed grounding");
                                session.last_rejection_reasons = violations;
                                had_violation = true;
                            }
                        }
                        None => {
                            // Defensive: the prompt tells the model to call
                            // propose_capstone_project immediately after
                            // propose_syllabus_plan, in the SAME turn — but never
                            // silently pass through a default/empty capstone if
                            // that didn't happen. Treated as a grounding failure
                            // so the normal retry mechanism (MAX_GROUNDING_RETRIES)
                            // corrects it instead of persisting an incomplete plan.
                            tracing::warn!(
                                session_id = %session.session_id,
                                "propose_syllabus_plan called without a matching propose_capstone_project in the same turn"
                            );
                            session.last_rejection_reasons = vec![
                                "propose_syllabus_plan se llamó sin propose_capstone_project en el mismo turno — ambas \
                                 herramientas deben llamarse juntas: primero propose_syllabus_plan y de inmediato después \
                                 propose_capstone_project, antes de terminar el turno."
                                    .to_string(),
                            ];
                            had_violation = true;
                        }
                    }
                }
                Some(_) => {
                    tracing::warn!(session_id = %session.session_id, "propose_syllabus_plan called before the diagnostic battery was answered");
                    session.last_rejection_reasons = vec![
                        "propose_syllabus_plan se llamó antes de que el estudiante respondiera la batería de diagnóstico \
                         (o antes de que entryLevel fuera absolute_zero)"
                            .to_string(),
                    ];
                    had_violation = true;
                }
                None => {
                    tracing::warn!(session_id = %session.session_id, "propose_syllabus_plan called before an assessment");
                    session.last_rejection_reasons = vec!["propose_syllabus_plan se llamó antes de submit_diagnostic_assessment".to_string()];
                    had_violation = true;
                }
            }
        }

        if capture.confirm_syllabus_plan.is_some() {
            match (session.learner_profile_card.clone(), session.proposed_plan.clone()) {
                (Some(profile), Some(plan)) => {
                    let battery = session.pending_diagnostic_battery.clone();
                    self.seal_session(session, &profile, plan.syllabus, battery)?;
                }
                _ => {
                    tracing::warn!(session_id = %session.session_id, "confirm_syllabus_plan called before a plan was proposed");
                    session.last_rejection_reasons = vec!["confirm_syllabus_plan se llamó antes de propose_syllabus_plan".to_string()];
                    had_violation = true;
                }
            }
        }

        Ok(had_violation)
    }

    /// Once-per-user-facing-turn bookkeeping for [`Self::MAX_GATHER_TURNS`] —
    /// split out from `apply_capture` so an internal grounding retry (see
    /// [`Self::run_diagnostic_turn_with_grounding_retry`]) doesn't spend
    /// more than one gather turn per real `advance`/`finish_diagnostic_stage`
    /// call, no matter how many model attempts it took internally.
    pub(super) fn record_gather_turn_if_unstable(session: &mut RoadmapSession) {
        let stable =
            session.status == SessionStatus::Sealed || session.pending_diagnostic_battery.is_some() || session.proposed_plan.is_some();
        if !stable {
            session.turn_count_in_phase += 1;
        }
    }

    /// Which agent/scope runs the MAIN turn of a real user-facing chat turn:
    /// no assessment captured yet → the one-tool `diagnostic_analyst`
    /// (Gate 1 only); once the profile exists → the full roadmap agent
    /// (confirm/revision turns). The battery/propose hand-offs never come
    /// from here — they're the follow-up turns in `advance`.
    pub(super) fn turn_agent_and_scope(session: &RoadmapSession) -> (&'static str, DiagnosticToolScope) {
        if session.learner_profile_card.is_none() {
            (ANALYST_AGENT_ID, DiagnosticToolScope::Assessment)
        } else {
            (ROADMAP_AGENT_ID, DiagnosticToolScope::All)
        }
    }

    /// Gate 2 follow-up: the assessment just landed (this or an earlier
    /// turn) but no battery exists yet, and the declared level isn't
    /// `absolute_zero` (which skips the battery by design). Runs as its own
    /// scoped turn right after a clean main turn instead of chaining two
    /// tools inside one prompt call.
    pub(super) fn battery_followup_needed(session: &RoadmapSession) -> bool {
        session.status == SessionStatus::Active
            && session.learner_profile_card.as_ref().is_some_and(|p| p.entry_level != EntryLevel::AbsoluteZero)
            && session.pending_diagnostic_battery.is_none()
    }

    /// Gate 3a follow-up: battery requirement satisfied (answered, or
    /// skipped via `absolute_zero`) but no proposal exists yet — the propose
    /// hand-off, normally driven by `finish_diagnostic_stage`, fires here
    /// for the paths where it runs inside `advance` (first capture).
    pub(super) fn propose_followup_needed(session: &RoadmapSession) -> bool {
        session.status == SessionStatus::Active
            && session.learner_profile_card.as_ref().is_some_and(|p| battery_satisfied(p, &session.pending_diagnostic_battery))
            && session.proposed_plan.is_none()
    }

    /// Runs the model, applies its capture, and — if that capture was
    /// rejected on content grounding — immediately retries WITHIN this one
    /// user-facing turn, up to [`Self::MAX_GROUNDING_RETRIES`] attempts,
    /// feeding the freshly recorded `last_rejection_reasons` back in via
    /// `render_input` each time. Without this, a grounding failure (now
    /// more likely on Gate 2's battery given its strict psychometric rules)
    /// left the student staring at a reply that promised a diagnostic/plan
    /// that never actually rendered, until they happened to send another
    /// message — the fix isn't supposed to depend on the student noticing
    /// and nudging it along.
    ///
    /// Returns `(text, had_violation)` — `had_violation` is whether grounding
    /// STILL failed after all attempts, which the caller feeds into
    /// `RoadmapSession::consecutive_grounding_failures` (see
    /// `Self::MAX_CONSECUTIVE_GROUNDING_FAILURES`): a real cap across turns,
    /// since this in-turn retry alone can't guarantee eventual success.
    pub(super) async fn run_diagnostic_turn_with_grounding_retry(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &mut RoadmapSession,
        agent_id: &str,
        scope: DiagnosticToolScope,
        render_input: impl Fn(&RoadmapSession) -> String,
    ) -> AppResult<(String, bool)> {
        let mut text = String::new();
        let mut had_violation = false;
        for attempt in 0..Self::MAX_GROUNDING_RETRIES {
            let input = render_input(session);
            let (t, capture) = self.run_diagnostic_turn_with_retry(app, agent_id, scope, &input).await?;
            text = t;
            had_violation = self.apply_capture(session, capture)?;
            if !had_violation || attempt + 1 >= Self::MAX_GROUNDING_RETRIES {
                break;
            }
        }
        if had_violation {
            // The model's OWN text (per its prompt, basically just the
            // closingQuestion/battery intro) still describes a battery/plan
            // that never actually validated on any attempt — showing that
            // verbatim is a broken promise: the student sees "¿Te parece
            // adecuada esta distribución?" with no distribution anywhere on
            // screen. Replace it with something honest and actionable —
            // `last_rejection_reasons` is already staged, so literally any
            // next message (even "sigue") retries with the fix in hand via
            // `render_turn_input`'s directive.
            text = "Casi lo tengo — dame un momento más para ajustarlo. Escríbeme de nuevo (puede ser solo \"sigue\") para continuar."
                .to_string();
        }
        Ok((text, had_violation))
    }

    /// Persists the confirmed syllabus into the relational store (course ->
    /// milestones -> classes), then seals the session. Shared by the normal
    /// tool-calling path and the force-close fallback — same shape either
    /// way, only where the `syllabus`/`diagnostic_battery` came from
    /// differs. No notebook content is generated/persisted here anymore —
    /// every class, including the first, gets its notebook lazily via
    /// `notebook_service::generation::start_class_notebook` the first time
    /// the student actually opens it (see `ConfirmSyllabusPlanArgs`).
    /// `diagnostic_battery` is `None` for the `absolute_zero` path (Gate 2
    /// was skipped, there's genuinely no battery to attach to the course) —
    /// otherwise carries whatever answers were actually recorded (real ones
    /// from the Diagnostic stage, or empty ones from force-close), both
    /// transferred onto the course atomically, never a fresh unanswered
    /// battery generated at seal time. The FIRST class is now scoped to the
    /// first micromodule of week 1 (see `import_syllabus_skeleton`), not the
    /// whole week.
    pub(super) fn seal_session(
        &self,
        session: &mut RoadmapSession,
        profile: &LearnerProfileCard,
        syllabus: RoadmapSyllabusPackage,
        diagnostic_battery: Option<DiagnosticBatteryState>,
    ) -> AppResult<()> {
        let (course_id, first_class_id, _) = self.import_syllabus_skeleton(&syllabus, &profile.target_goal)?;
        // Course-scoped, not a notebook block: lets the battery shape the
        // syllabus and every class generated within it, not just be content
        // read once inside "a class" — see `domain::roadmap::
        // ProposeSyllabusPlanArgs`.
        if let Some(battery) = &diagnostic_battery {
            self.notebook_store.set_course_diagnostic_battery(&course_id, &battery.battery)?;
            if !battery.answers.is_empty() {
                self.notebook_store.save_diagnostic_battery_answers(&course_id, &battery.answers)?;
            }
        }

        session.imported_course_id = Some(course_id);
        session.first_class_id = first_class_id;
        session.roadmap_package =
            Some(build_sealed_roadmap(session, profile.clone(), syllabus, diagnostic_battery.map(|b| b.battery)));
        session.pending_diagnostic_battery = None;
        session.proposed_plan = None;
        session.status = SessionStatus::Sealed;
        session.phase = RoadmapPhase::Roadmap;
        session.last_rejection_reasons.clear();
        Ok(())
    }

    /// Creates the course/milestones/classes skeleton in the notebook store
    /// (no notebook content yet) — the part of sealing that's shared with
    /// [`Self::ensure_course_imported`]'s recovery path. Returns
    /// `(course_id, first_class_id, first_class_title)`. Delegates to
    /// `notebook_store::import_syllabus_into_store` — the same function
    /// `NotebookService::import_course_from_roadmap` calls, so the two
    /// import paths (seal-time vs. on-demand recovery) can't drift apart.
    pub(super) fn import_syllabus_skeleton(
        &self,
        syllabus: &RoadmapSyllabusPackage,
        target_goal: &str,
    ) -> AppResult<(String, Option<String>, String)> {
        crate::notebook_store::import_syllabus_into_store(&self.notebook_store, syllabus, target_goal)
    }

    /// Idempotently makes sure this SEALED session's course/classes exist in
    /// the SQLite store, persisting `imported_course_id`/`first_class_id`
    /// back onto the session so it only ever runs once. Recovery path for a
    /// session sealed before this import existed (or where it's otherwise
    /// missing) — without this, "Ver tu primera clase" has nothing to
    /// navigate to and silently does nothing. Doesn't generate the first
    /// class's notebook content itself: `NotebookService::
    /// start_class_notebook` already does that lazily the first time the
    /// class is opened (same path any other week uses), so there's no need
    /// to duplicate that here.
    pub fn ensure_course_imported(&self, session_id: &str) -> AppResult<RoadmapSession> {
        let mut session = self.get_session(session_id)?;
        if session.imported_course_id.is_some() && session.first_class_id.is_some() {
            return Ok(session);
        }
        let package = session
            .roadmap_package
            .clone()
            .ok_or_else(|| AppError::InvalidInput("session has no sealed syllabus to import".to_string()))?;

        let (course_id, first_class_id, _) =
            self.import_syllabus_skeleton(&package.syllabus, &package.learner_profile.target_goal)?;
        session.imported_course_id = Some(course_id);
        session.first_class_id = first_class_id;
        self.store.save_roadmap_session(&session)?;
        Ok(session)
    }

    /// The hard backstop: called only once `session.turn_count_in_phase`
    /// has already reached [`Self::MAX_GATHER_TURNS`] — the model was given
    /// every turn the prompt promised and never completed both tool calls.
    /// Synthesizes honest, generic defaults for whatever's still missing
    /// and seals anyway, so the conversation can never stall past its cap.
    /// No model call is spent on this turn.
    pub(super) fn force_close_session(&self, session: &mut RoadmapSession) -> AppResult<String> {
        if session.learner_profile_card.is_none() {
            session.learner_profile_card = Some(synth_profile_card(&session.draft));
            session.diagnostic_summary_card = Some(synth_diagnostic_card());
        }
        let profile = session.learner_profile_card.clone().expect("just ensured above");
        let syllabus = synth_roadmap_package(&profile);
        // Guaranteed None here: once a battery is successfully generated,
        // `apply_capture` stops counting gather turns against it (see its
        // `stable` check), so the cap that triggers force-close can only be
        // reached while there's no pending battery yet.
        let diagnostic_battery = Some(DiagnosticBatteryState { battery: synth_diagnostic_battery(&profile), answers: Default::default() });
        if let Err(e) = self.seal_session(session, &profile, syllabus, diagnostic_battery) {
            tracing::warn!(session_id = %session.session_id, error = %e, "force-close failed to persist into the notebook store");
            return Err(e);
        }
        Ok("¡Listo! Con lo que me compartiste armo tu plan ahora mismo — tu primera clase se genera en cuanto la abras.".to_string())
    }

    /// The OTHER hard backstop — mirrors `force_close_session`, but for Gate
    /// 3 instead of Gate 1/2: called once `consecutive_grounding_failures`
    /// reaches [`Self::MAX_CONSECUTIVE_GROUNDING_FAILURES`], i.e. the battery
    /// is already answered but the model can't land a valid
    /// `propose_syllabus_plan`/`confirm_syllabus_plan` (or keeps confusedly
    /// re-presenting the finished battery) across several REAL turns in a
    /// row. Unlike `force_close_session`, this reuses whatever real progress
    /// already exists — the actual answered battery, and a prior successful
    /// proposal if one exists — instead of synthesizing everything from
    /// scratch, since discarding a genuinely valid proposal just because a
    /// LATER turn failed would be wasteful.
    pub(super) fn force_progress_from_gate3(&self, session: &mut RoadmapSession) -> AppResult<String> {
        let Some(profile) = session.learner_profile_card.clone() else {
            // Defensive: should be unreachable (ceiling #1 in `advance`
            // always fires first while there's no profile yet), but never
            // panic a real request over an internal invariant — fall back
            // to the Gate 1/2 backstop instead.
            return self.force_close_session(session);
        };
        let syllabus = session.proposed_plan.as_ref().map(|p| p.syllabus.clone()).unwrap_or_else(|| synth_roadmap_package(&profile));
        let diagnostic_battery = session.pending_diagnostic_battery.clone();
        if let Err(e) = self.seal_session(session, &profile, syllabus, diagnostic_battery) {
            tracing::warn!(session_id = %session.session_id, error = %e, "force-progress (Gate 3) failed to persist into the notebook store");
            return Err(e);
        }
        Ok("¡Listo! Con lo que ya tenemos armo tu plan ahora mismo — tu primera clase se genera en cuanto la abras.".to_string())
    }
}
