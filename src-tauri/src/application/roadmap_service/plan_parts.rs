//! The syllabus proposal written IN PARTS (see `domain::plan_parts`): an outline
//! first, then the sessions of every week with a few weeks in flight at once, each
//! part announced to the UI the moment it exists so the learner watches the plan
//! appear instead of staring at a spinner for minutes.
//!
//! The parts are assembled into the same `propose_syllabus_plan` +
//! `propose_capstone_project` capture the single-call flow produced and go
//! through the same [`RoadmapService::apply_capture`] grounding, so everything
//! downstream (proposal, adjustment, sealing) is unchanged. Each part is
//! validated on its own and retried with the reasons, which keeps one bad week
//! from throwing away the rest.

use std::sync::Mutex;

use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use tauri::Emitter;

use crate::agents::plan_outline_agent::PLAN_OUTLINE_AGENT_ID;
use crate::agents::plan_week_agent::PLAN_WEEK_AGENT_ID;
use crate::domain::plan_parts::{OutlineWeek, PlanOutlineArgs, assemble_week, session_hours};
use crate::domain::roadmap::{CapstoneProjectArgs, ProposeSyllabusPlanArgs};
use crate::tools::{DiagnosticToolScope, ResearchCapture, ResearchStep, RoadmapCapture, SharedResearchCapture};

use super::grounding::*;
use super::render::*;
use super::*;

/// Event the UI listens to while a proposal is being written.
pub const PLAN_PART_EVENT: &str = "roadmap://plan_part";

/// Weeks written at the same time. Three keeps the wall-clock close to
/// `ceil(weeks / 3)` single-week calls without hammering the provider's rate limit.
const WEEK_CONCURRENCY: usize = 3;

/// Attempts at one part: a real try plus one correction told the reasons.
const MAX_PART_ATTEMPTS: u8 = 2;

/// At most this many reasons are fed back / stored, so a pathological proposal
/// cannot turn the next prompt into a wall of text.
const MAX_REASONS: usize = 8;

/// What the UI is told. Only student-facing text: no block catalog, no internals.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
enum PlanPart {
    /// A new attempt starts: whatever was shown before is stale.
    Started,
    Outline { course_title: String, core_focus: String, weeks: Vec<OutlineWeekView>, capstone_title: String },
    Week { week: u16, sessions: Vec<SessionView> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutlineWeekView {
    week: u16,
    title: String,
    weekly_goal: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionView {
    label: String,
    hours: f32,
    objective: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanPartEvent<'a> {
    session_id: &'a str,
    #[serde(flatten)]
    part: PlanPart,
}

fn emit_part(app: Option<&tauri::AppHandle>, session_id: &str, part: PlanPart) {
    if let Some(handle) = app {
        // Best-effort, like every other progress event: never fails a run.
        let _ = handle.emit(PLAN_PART_EVENT, &PlanPartEvent { session_id, part });
    }
}

/// Outcome of writing every part: the assembled capture, or why it was refused.
enum PartsOutcome {
    Plan { text: String, capture: RoadmapCapture },
    Rejected(Vec<String>),
}

/// Content-level checks for the outline (what `syllabus_violations` would say
/// about the same fields, plus the week count the parts depend on).
pub(super) fn outline_violations(profile: &LearnerProfileCard, outline: &PlanOutlineArgs) -> Vec<String> {
    let mut v = Vec::new();
    if outline.course_title.trim().is_empty() {
        v.push("courseTitle está vacío".to_string());
    } else {
        check_student_facing_text(&mut v, "courseTitle", &outline.course_title);
    }
    for (field, empty) in [
        ("coreFocus", outline.core_focus.trim().is_empty()),
        ("identifiedNeeds", outline.identified_needs.is_empty()),
        ("learningStrategy", outline.learning_strategy.trim().is_empty()),
        ("closingQuestion", outline.closing_question.trim().is_empty()),
    ] {
        if empty {
            v.push(format!("{field} está vacío"));
        }
    }
    if outline.weeks.len() != profile.timeframe_weeks as usize {
        v.push(format!("weeks tiene {} semanas pero el estudiante pidió {}", outline.weeks.len(), profile.timeframe_weeks));
    }
    let mut numbers: Vec<u16> = outline.weeks.iter().map(|w| w.week).collect();
    numbers.sort_unstable();
    if numbers != (1..=outline.weeks.len() as u16).collect::<Vec<_>>() {
        v.push(format!("los números de semana ({numbers:?}) no son 1..N consecutivos y sin repetir"));
    }
    for w in &outline.weeks {
        if w.title.trim().is_empty() {
            v.push(format!("la semana {} tiene título vacío", w.week));
        } else {
            check_student_facing_text(&mut v, &format!("título de la semana {}", w.week), &w.title);
        }
        if w.deliverable.trim().is_empty() {
            v.push(format!("la semana {} tiene entregable vacío", w.week));
        } else if is_vague_deliverable(&w.deliverable) {
            v.push(format!("el entregable de la semana {} (\"{}\") es vago — debe ser un artefacto verificable", w.week, w.deliverable));
        } else {
            check_student_facing_text(&mut v, &format!("entregable de la semana {}", w.week), &w.deliverable);
        }
        match w.weekly_goal.as_deref().map(str::trim) {
            None | Some("") => v.push(format!("la semana {} no tiene weeklyGoal (qué problema resuelve y qué capacidad desbloquea)", w.week)),
            Some(goal) if is_vague_deliverable(goal) => v.push(format!("el weeklyGoal de la semana {} (\"{goal}\") es vago", w.week)),
            Some(goal) => check_student_facing_text(&mut v, &format!("weeklyGoal de la semana {}", w.week), goal),
        }
    }
    v.extend(capstone_project_violations(&outline.capstone_project));
    v
}

impl RoadmapService {
    /// Writes the syllabus proposal in parts instead of one giant call. Off by
    /// default so the single-call flow (and its tests) stay as they are; the app
    /// turns it on.
    pub fn with_plan_in_parts(mut self) -> Self {
        self.plan_in_parts = true;
        self
    }

    /// The propose turn behind every proposal path (the battery's last answer,
    /// the `absolute_zero` form, a plan adjustment). Same contract as
    /// [`Self::run_diagnostic_turn_with_grounding_retry`] with the `Propose`
    /// scope — it is that function unless the plan is written in parts.
    pub(super) async fn run_propose_turn(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &mut RoadmapSession,
        adjust: Option<(&ProposedPlan, &PlanAdjustment)>,
    ) -> AppResult<(String, bool)> {
        if !self.plan_in_parts {
            return self
                .run_diagnostic_turn_with_grounding_retry(app, session, ROADMAP_AGENT_ID, DiagnosticToolScope::Propose, |s| match s.learner_profile_card.as_ref() {
                    Some(profile) => match adjust {
                        Some((previous, adjustment)) => render_adjusted_propose_input(profile, s.pending_diagnostic_battery.as_ref(), s.learning_context.as_ref(), previous, adjustment, &s.last_rejection_reasons),
                        None => render_propose_syllabus_input(profile, s.pending_diagnostic_battery.as_ref(), s.learning_context.as_ref(), &s.last_rejection_reasons),
                    },
                    None => String::new(),
                })
                .await;
        }

        let mut text = String::new();
        let mut had_violation = false;
        for attempt in 0..Self::MAX_GROUNDING_RETRIES {
            match self.write_plan_in_parts(app, session, adjust).await? {
                PartsOutcome::Plan { text: t, capture } => {
                    text = t;
                    had_violation = self.apply_capture(session, capture)?;
                }
                PartsOutcome::Rejected(reasons) => {
                    crate::dev_trace::trace_note("critique", "roadmap_grounding", &session.session_id, "plan_parts_rejected", &reasons);
                    tracing::warn!(session_id = %session.session_id, ?reasons, "the syllabus written in parts failed validation");
                    session.last_rejection_reasons = reasons;
                    had_violation = true;
                }
            }
            if !had_violation || attempt + 1 >= Self::MAX_GROUNDING_RETRIES {
                break;
            }
        }
        if had_violation {
            // The same honest fallback the single-call flow uses: nothing the
            // model wrote validated, so none of it may reach the learner.
            text = "Casi lo tengo — dame un momento más para ajustarlo. Escríbeme de nuevo (puede ser solo \"sigue\") para continuar.".to_string();
        }
        Ok((text, had_violation))
    }

    /// One model call for one part, retrying only transient failures (the model
    /// produced nothing, so the same input is simply sent again).
    async fn run_part(&self, app: Option<&tauri::AppHandle>, agent_id: &str, input: &str, step: ResearchStep) -> AppResult<ResearchCapture> {
        let mut last_err = None;
        for attempt in 0..Self::MAX_EXECUTION_ATTEMPTS {
            let capture: SharedResearchCapture = Arc::new(Mutex::new(ResearchCapture::default()));
            match self.orchestrator.run_research_agent(app, agent_id, input, capture.clone(), step).await {
                Ok(_) => return Ok(capture.lock().unwrap_or_else(|e| e.into_inner()).clone()),
                Err(e) if (e.is_transient() || e.is_output_budget_exhausted()) && attempt + 1 < Self::MAX_EXECUTION_ATTEMPTS => {
                    let backoff = Self::RETRY_BACKOFF_BASE * (attempt as u32 + 1);
                    tracing::warn!(error = %e, attempt, agent_id, backoff_ms = backoff.as_millis(), "plan part failed, retrying after backoff");
                    last_err = Some(e);
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.expect("loop only exits None via an Err branch above"))
    }

    /// The outline, validated; `Ok(Err(reasons))` when the model could not
    /// produce a valid one in [`MAX_PART_ATTEMPTS`] tries.
    async fn write_outline(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &RoadmapSession,
        profile: &LearnerProfileCard,
        adjust: Option<(&ProposedPlan, &PlanAdjustment)>,
    ) -> AppResult<Result<PlanOutlineArgs, Vec<String>>> {
        let mut feedback = session.last_rejection_reasons.clone();
        for _ in 0..MAX_PART_ATTEMPTS {
            let input = render_outline_input(profile, session.pending_diagnostic_battery.as_ref(), session.learning_context.as_ref(), adjust, &feedback);
            let captured = self.run_part(app, PLAN_OUTLINE_AGENT_ID, &input, ResearchStep::Outline).await?;
            match captured.outline {
                Some(outline) => {
                    let violations = outline_violations(profile, &outline);
                    if violations.is_empty() {
                        return Ok(Ok(outline));
                    }
                    feedback = violations;
                }
                None => feedback = vec!["No llamaste a submit_plan_outline: llámala ahora con el esquema.".to_string()],
            }
        }
        Ok(Err(feedback))
    }

    /// The sessions of ONE week, validated. The number of sessions and their
    /// hours are decided here, so the only things left to check are content.
    #[allow(clippy::too_many_arguments)]
    async fn write_week(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &RoadmapSession,
        profile: &LearnerProfileCard,
        adjust: Option<(&ProposedPlan, &PlanAdjustment)>,
        outline: &PlanOutlineArgs,
        week: &OutlineWeek,
    ) -> AppResult<Result<Milestone, Vec<String>>> {
        let pace = profile.weekly_commitment_hours;
        let hours = session_hours(pace, session_count_bounds(pace).0);
        let mut feedback: Vec<String> = Vec::new();
        for _ in 0..MAX_PART_ATTEMPTS {
            let input = render_week_input(profile, session.pending_diagnostic_battery.as_ref(), session.learning_context.as_ref(), adjust, outline, week, &hours, &feedback);
            let captured = self.run_part(app, PLAN_WEEK_AGENT_ID, &input, ResearchStep::Week).await?;
            let Some(args) = captured.week else {
                feedback = vec!["No llamaste a submit_week_sessions: llámala ahora con las sesiones.".to_string()];
                continue;
            };
            if args.sessions.len() != hours.len() {
                feedback = vec![format!("la semana {} necesita EXACTAMENTE {} sesiones y entregaste {}", week.week, hours.len(), args.sessions.len())];
                continue;
            }
            let milestone = assemble_week(week, args.sessions, &hours);
            let violations = micromodule_violations(week.week, pace, &milestone.micromodules);
            if violations.is_empty() {
                return Ok(Ok(milestone));
            }
            feedback = violations;
        }
        Ok(Err(feedback))
    }

    /// [`Self::write_week`] plus telling the UI as soon as the week exists.
    async fn write_and_announce_week(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &RoadmapSession,
        profile: &LearnerProfileCard,
        adjust: Option<(&ProposedPlan, &PlanAdjustment)>,
        outline: &PlanOutlineArgs,
        week: &OutlineWeek,
    ) -> AppResult<Result<Milestone, Vec<String>>> {
        let result = self.write_week(app, session, profile, adjust, outline, week).await;
        if let Ok(Ok(milestone)) = &result {
            emit_part(
                app,
                &session.session_id,
                PlanPart::Week {
                    week: milestone.week,
                    sessions: milestone
                        .micromodules
                        .iter()
                        .map(|m| SessionView { label: m.label.clone(), hours: m.hours, objective: m.objective.clone() })
                        .collect(),
                },
            );
        }
        result
    }

    /// Writes the outline, then every week (a few at a time), announcing each part
    /// as it lands, and assembles the capture the rest of the flow expects.
    async fn write_plan_in_parts(
        &self,
        app: Option<&tauri::AppHandle>,
        session: &RoadmapSession,
        adjust: Option<(&ProposedPlan, &PlanAdjustment)>,
    ) -> AppResult<PartsOutcome> {
        let profile = session
            .learner_profile_card
            .clone()
            .ok_or_else(|| AppError::InvalidInput("session has no diagnostic assessment yet".to_string()))?;
        let session_id = session.session_id.as_str();
        emit_part(app, session_id, PlanPart::Started);

        let outline = match self.write_outline(app, session, &profile, adjust).await? {
            Ok(outline) => outline,
            Err(reasons) => return Ok(PartsOutcome::Rejected(cap_reasons(reasons))),
        };
        let mut shown = outline.weeks.clone();
        shown.sort_by_key(|w| w.week);
        emit_part(
            app,
            session_id,
            PlanPart::Outline {
                course_title: outline.course_title.clone(),
                core_focus: outline.core_focus.clone(),
                weeks: shown.iter().map(|w| OutlineWeekView { week: w.week, title: w.title.clone(), weekly_goal: w.weekly_goal.clone() }).collect(),
                capstone_title: outline.capstone_project.title.clone(),
            },
        );

        // At most WEEK_CONCURRENCY weeks in flight, refilled as each one lands.
        // (Hand-rolled instead of `buffer_unordered`: a closure returning a future
        // that borrows `&self`/`&AppHandle` trips rustc's "FnOnce is not general
        // enough" inside a `#[tauri::command]` future.)
        let mut results: Vec<AppResult<Result<Milestone, Vec<String>>>> = Vec::new();
        {
            let mut in_flight = FuturesUnordered::new();
            let mut waiting = shown.iter();
            loop {
                while in_flight.len() < WEEK_CONCURRENCY {
                    let Some(week) = waiting.next() else { break };
                    in_flight.push(self.write_and_announce_week(app, session, &profile, adjust, &outline, week));
                }
                match in_flight.next().await {
                    Some(result) => results.push(result),
                    None => break,
                }
            }
        } // the futures borrow `outline`; they are gone before it is moved below

        let mut milestones = Vec::new();
        let mut reasons = Vec::new();
        for result in results {
            match result? {
                Ok(milestone) => milestones.push(milestone),
                Err(week_reasons) => reasons.extend(week_reasons),
            }
        }
        if !reasons.is_empty() {
            return Ok(PartsOutcome::Rejected(cap_reasons(reasons)));
        }
        milestones.sort_by_key(|m| m.week);

        let capstone = outline.capstone_project.clone();
        let text = outline.closing_question.trim().to_string();
        let capture = RoadmapCapture {
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: outline.core_focus,
                identified_needs: outline.identified_needs,
                learning_strategy: outline.learning_strategy,
                syllabus: RoadmapSyllabusPackage {
                    course_title: outline.course_title,
                    total_weeks: profile.timeframe_weeks,
                    pace_hours_per_week: profile.weekly_commitment_hours,
                    milestones,
                    capstone_project: capstone.clone(),
                },
                closing_question: outline.closing_question,
            }),
            propose_capstone_project: Some(CapstoneProjectArgs { capstone_project: capstone }),
            ..Default::default()
        };
        Ok(PartsOutcome::Plan { text, capture })
    }
}

fn cap_reasons(mut reasons: Vec<String>) -> Vec<String> {
    reasons.truncate(MAX_REASONS);
    reasons
}
