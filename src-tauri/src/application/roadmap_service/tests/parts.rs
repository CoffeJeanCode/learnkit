//! The syllabus written in parts: an outline, then the sessions of every week
//! with a few weeks in flight at once. What must hold: the assembled plan is the
//! same one the single-call flow produces, each part is validated and retried on
//! its own, hours are decided by code, and the parallelism has a ceiling.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::harness::*;
use super::*;
use crate::agents::{plan_outline_agent, plan_week_agent};
use crate::domain::plan_parts::{OutlineWeek, PlanOutlineArgs, SessionDraft, WeekSessionsArgs};
use crate::domain::roadmap::{PlanAdjustment, PlanEmphasis};

type WeekScript = Box<dyn FnMut(u16, usize, u32) -> AppResult<Option<WeekSessionsArgs>> + Send>;

/// Scripts the two plan-part agents. The outline answers come from a queue; a
/// week's answer comes from a closure of `(week, session count, attempt for that week)`.
struct PartsRunner {
    outlines: Mutex<VecDeque<AppResult<Option<PlanOutlineArgs>>>>,
    week_script: Mutex<WeekScript>,
    week_attempts: Mutex<HashMap<u16, u32>>,
    outline_inputs: Mutex<Vec<String>>,
    week_inputs: Mutex<Vec<String>>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    /// Pause inside every week call, so concurrent weeks actually overlap.
    week_delay_ms: u64,
}

impl PartsRunner {
    fn new(outlines: Vec<AppResult<Option<PlanOutlineArgs>>>, week_script: WeekScript) -> Arc<Self> {
        Arc::new(Self {
            outlines: Mutex::new(outlines.into_iter().collect()),
            week_script: Mutex::new(week_script),
            week_attempts: Mutex::new(HashMap::new()),
            outline_inputs: Mutex::new(vec![]),
            week_inputs: Mutex::new(vec![]),
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
            week_delay_ms: 0,
        })
    }

    fn with_delay(self: Arc<Self>, ms: u64) -> Arc<Self> {
        let mut inner = Arc::try_unwrap(self).ok().expect("not shared yet");
        inner.week_delay_ms = ms;
        Arc::new(inner)
    }

    fn outline_calls(&self) -> usize {
        self.outline_inputs.lock().unwrap().len()
    }

    fn week_calls(&self) -> usize {
        self.week_inputs.lock().unwrap().len()
    }
}

/// "Escribe las 3 sesiones de la semana 5 llamando…" -> (5, 3). The renderer puts
/// both numbers on the first line precisely so a request is unambiguous.
fn parse_week_request(input: &str) -> (u16, usize) {
    let first = input.lines().next().unwrap_or_default();
    let after = |marker: &str| -> u32 {
        let rest = &first[first.find(marker).expect("marker") + marker.len()..];
        rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().expect("number")
    };
    let week = after("de la semana ") as u16;
    let count = first.strip_prefix("Escribe las ").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).expect("count");
    (week, count)
}

#[async_trait]
impl PromptRunner for PartsRunner {
    async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
        unreachable!()
    }

    async fn run_diagnostic_execution(&self, _: &str, _: &str, _: &str, _: &str, _: SharedRoadmapCapture, _: DiagnosticToolScope) -> AppResult<PromptOutput> {
        unreachable!("with the plan in parts, the proposal never uses the single-call turn")
    }

    async fn run_research_execution(
        &self,
        _: &str,
        _: &str,
        _: &str,
        input: &str,
        capture: SharedResearchCapture,
        step: ResearchStep,
    ) -> AppResult<PromptOutput> {
        match step {
            ResearchStep::Outline => {
                self.outline_inputs.lock().unwrap().push(input.to_string());
                if let Some(args) = self.outlines.lock().unwrap().pop_front().expect("scripted outline available")? {
                    capture.lock().unwrap().outline = Some(args);
                }
            }
            ResearchStep::Week => {
                self.week_inputs.lock().unwrap().push(input.to_string());
                let (week, count) = parse_week_request(input);
                let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_in_flight.fetch_max(now, Ordering::SeqCst);
                if self.week_delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(self.week_delay_ms)).await;
                }
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                let attempt = {
                    let mut attempts = self.week_attempts.lock().unwrap();
                    let n = attempts.entry(week).or_insert(0);
                    *n += 1;
                    *n
                };
                if let Some(args) = (self.week_script.lock().unwrap())(week, count, attempt)? {
                    capture.lock().unwrap().week = Some(args);
                }
            }
            other => panic!("the plan parts only drive the outline and week steps, got {other:?}"),
        }
        Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
    }
}

fn outline(weeks: u16) -> PlanOutlineArgs {
    PlanOutlineArgs {
        course_title: "Mutaciones genéticas".to_string(),
        core_focus: "Entender cómo ocurre una mutación".to_string(),
        identified_needs: vec!["Practicar con ejemplos reales".to_string()],
        learning_strategy: "Guiado, con práctica en cada sesión".to_string(),
        weeks: (1..=weeks)
            .map(|week| OutlineWeek {
                week,
                title: format!("Semana {week}"),
                deliverable: "Entregable de la semana".to_string(),
                weekly_goal: Some("Avanzar un paso concreto hacia la meta declarada".to_string()),
            })
            .collect(),
        capstone_project: CapstoneProject {
            title: "Proyecto terminal".to_string(),
            description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
            verifiable_evidence: "Informe con tres casos analizados + presentación grabada".to_string(),
        },
        closing_question: "¿Te parece adecuada esta distribución?".to_string(),
    }
}

fn sessions(count: usize) -> WeekSessionsArgs {
    WeekSessionsArgs {
        sessions: (0..count)
            .map(|i| SessionDraft {
                label: format!("Días {}-{} — Sesión {}", 2 * i + 1, 2 * i + 2, i + 1),
                focus: Some(format!("Conceptos centrales de la sesión {}", i + 1)),
                deliverable: Deliverable { artifact_type: DeliverableArtifactType::TestsPassing, description: format!("Artefacto verificable {}", i + 1) },
                objective: Some(format!("Explicar el concepto central de la sesión {} con un ejemplo propio", i + 1)),
                interactive_blocks: vec!["socratic_prediction".to_string(), "hands_on_mission".to_string(), "metacognitive_closure".to_string()],
            })
            .collect(),
    }
}

/// Every week answers correctly on the first try.
fn always_valid() -> WeekScript {
    Box::new(|_, count, _| Ok(Some(sessions(count))))
}

fn parts_harness(runner: Arc<PartsRunner>) -> RoadmapServiceHarness {
    let agents = Arc::new(AgentRegistry::new());
    agents.register(crate::agents::roadmap_agent::definition()).expect("roadmap agent");
    agents.register(plan_outline_agent::definition()).expect("outline agent");
    agents.register(plan_week_agent::definition()).expect("week agent");
    let orchestrator = Arc::new(Orchestrator::new(agents, runner));
    let dir = std::env::temp_dir().join(format!("learnkit-roadmap-parts-{}", Uuid::new_v4()));
    let service = RoadmapService::new(orchestrator, FileStore::new(dir.clone()), NotebookStore::open_in_memory().expect("store")).with_plan_in_parts();
    RoadmapServiceHarness { service, dir }
}

/// A session standing right before the proposal: an `absolute_zero` profile, so
/// no battery stands in the way.
async fn ready_for_proposal(harness: &RoadmapServiceHarness, weeks: u16, hours: f32) -> RoadmapSession {
    let mut session = harness.service.start_session(None).await.expect("start").session;
    session.learner_profile_card = Some(super::super::grounding::build_profile_card(&full_assessment("Mutaciones genéticas", weeks, hours, EntryLevel::AbsoluteZero)));
    session
}

fn cleanup(harness: &RoadmapServiceHarness) {
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test]
async fn the_plan_is_assembled_from_one_outline_and_one_call_per_week() {
    let runner = PartsRunner::new(vec![Ok(Some(outline(4)))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 4, 6.0).await;

    let (text, violated) = harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off").expect("a proposal was due");

    assert!(!violated, "reasons: {:?}", session.last_rejection_reasons);
    assert_eq!(text, "¿Te parece adecuada esta distribución?", "the learner reads the closing question");
    assert_eq!((runner.outline_calls(), runner.week_calls()), (1, 4), "one outline + one call per week, nothing else");
    let plan = session.proposed_plan.as_ref().expect("a proposal is pending");
    assert_eq!(plan.syllabus.milestones.iter().map(|m| m.week).collect::<Vec<_>>(), [1, 2, 3, 4], "in week order whatever order they finished in");
    assert!(plan.syllabus.milestones.iter().all(|m| m.micromodules.len() == 2), "6 h/week is two sessions");
    assert!(plan.syllabus.milestones.iter().flat_map(|m| &m.micromodules).all(|s| s.hours == 3.0), "hours come from code: 6 h over 2 sessions");
    assert_eq!(plan.syllabus.pace_hours_per_week, 6.0);
    assert_eq!(plan.syllabus.capstone_project.title, "Proyecto terminal");
    assert_eq!(plan.diagnostic_summary.core_focus, "Entender cómo ocurre una mutación");
    cleanup(&harness);
}

#[tokio::test]
async fn more_weekly_hours_mean_more_sessions_and_still_at_most_four_hours_each() {
    let runner = PartsRunner::new(vec![Ok(Some(outline(2)))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 2, 12.0).await;

    harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off");

    let plan = session.proposed_plan.as_ref().expect("proposal");
    for m in &plan.syllabus.milestones {
        assert_eq!(m.micromodules.iter().map(|s| s.hours).collect::<Vec<_>>(), [4.0, 4.0, 4.0], "12 h/week is three 4 h sessions");
    }
    assert!(runner.week_inputs.lock().unwrap().iter().all(|i| i.starts_with("Escribe las 3 sesiones de la semana ")), "the model is told how many sessions to write");
    cleanup(&harness);
}

#[tokio::test]
async fn a_week_with_the_wrong_number_of_sessions_is_retried_with_the_reason_and_the_others_are_untouched() {
    let script: WeekScript = Box::new(|week, count, attempt| Ok(Some(sessions(if week == 2 && attempt == 1 { count + 1 } else { count }))));
    let runner = PartsRunner::new(vec![Ok(Some(outline(3)))], script);
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 3, 6.0).await;

    let (_, violated) = harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off").expect("due");

    assert!(!violated && session.proposed_plan.is_some());
    assert_eq!(runner.week_calls(), 4, "3 weeks + ONE retry for week 2, not a full rewrite");
    let retry = runner.week_inputs.lock().unwrap().iter().find(|i| i.contains("EXACTAMENTE 2 sesiones")).cloned();
    assert!(retry.is_some(), "the retry is told what was wrong");
    cleanup(&harness);
}

#[tokio::test]
async fn a_week_that_never_validates_rejects_the_proposal_and_nothing_half_written_survives() {
    // Week 2 always returns a session with a vague deliverable.
    let script: WeekScript = Box::new(|week, count, _| {
        let mut s = sessions(count);
        if week == 2 {
            s.sessions[0].deliverable.description = "comprender la teoría".to_string();
        }
        Ok(Some(s))
    });
    let runner = PartsRunner::new(vec![Ok(Some(outline(3)))], script);
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 3, 6.0).await;

    let (text, violated) = harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off").expect("due");

    assert!(violated, "a bad week is a grounding failure like any other");
    assert!(session.proposed_plan.is_none(), "an incomplete plan is never kept");
    assert!(session.last_rejection_reasons.iter().any(|r| r.contains("semana 2")), "{:?}", session.last_rejection_reasons);
    assert!(text.contains("Casi lo tengo"), "the learner never sees text for a plan that does not exist: {text}");
    cleanup(&harness);
}

#[tokio::test]
async fn an_outline_with_the_wrong_number_of_weeks_is_retried_and_never_reaches_the_week_step() {
    let runner = PartsRunner::new(vec![Ok(Some(outline(2))), Ok(Some(outline(2))), Ok(Some(outline(2))), Ok(Some(outline(2)))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 4, 6.0).await;

    let (_, violated) = harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off").expect("due");

    assert!(violated && session.proposed_plan.is_none());
    assert_eq!(runner.week_calls(), 0, "no week is written against an outline that is wrong");
    assert!(runner.outline_inputs.lock().unwrap()[1].contains("pidió 4"), "the retry says how many weeks were asked for");
    cleanup(&harness);
}

#[tokio::test]
async fn a_transient_failure_in_a_part_is_retried_without_losing_the_rest() {
    let script: WeekScript = Box::new(|week, count, attempt| {
        if week == 3 && attempt == 1 {
            return Err(AppError::ProviderConnectionFailed("blip".to_string()));
        }
        Ok(Some(sessions(count)))
    });
    let runner = PartsRunner::new(vec![Ok(Some(outline(3)))], script);
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 3, 6.0).await;

    let (_, violated) = harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off").expect("due");

    assert!(!violated && session.proposed_plan.is_some());
    assert_eq!(runner.week_calls(), 4, "the failed week was asked again, the others were not");
    cleanup(&harness);
}

#[tokio::test]
async fn a_permanent_failure_surfaces_as_an_error_not_as_a_silent_empty_plan() {
    let runner = PartsRunner::new(vec![Err(AppError::ProviderKeyMissing("anthropic".to_string()))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 3, 6.0).await;

    let err = harness.service.run_followup_hand_off(None, &mut session).await.expect_err("no key, no plan");

    assert!(matches!(err, AppError::ProviderKeyMissing(_)), "{err:?}");
    assert_eq!(runner.week_calls(), 0);
    cleanup(&harness);
}

#[tokio::test]
async fn at_most_three_weeks_are_written_at_once_and_they_do_overlap() {
    let runner = PartsRunner::new(vec![Ok(Some(outline(8)))], always_valid()).with_delay(40);
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 8, 6.0).await;

    harness.service.run_followup_hand_off(None, &mut session).await.expect("hand-off");

    assert_eq!(runner.week_calls(), 8);
    assert_eq!(runner.max_in_flight.load(Ordering::SeqCst), 3, "a ceiling of three, and the ceiling is actually used");
    assert_eq!(session.proposed_plan.as_ref().expect("plan").syllabus.milestones.len(), 8);
    cleanup(&harness);
}

#[tokio::test]
async fn adjusting_a_plan_rewrites_it_in_parts_and_tells_the_model_what_changed() {
    let runner = PartsRunner::new(vec![Ok(Some(outline(4))), Ok(Some(outline(2)))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 4, 6.0).await;
    harness.service.run_followup_hand_off(None, &mut session).await.expect("first proposal");
    harness.service.store.save_roadmap_session(&session).expect("save");
    let sid = session.session_id.clone();

    let adjustment = PlanAdjustment { timeframe_weeks: Some(2), emphasis: Some(PlanEmphasis::MorePractice), ..Default::default() };
    let r = harness.service.adjust_proposed_plan(None, &sid, adjustment).await.expect("adjust");

    let plan = r.session.proposed_plan.as_ref().expect("a revised proposal");
    assert_eq!(plan.syllabus.milestones.len(), 2);
    let input = runner.outline_inputs.lock().unwrap()[1].clone();
    assert!(input.contains("previousProposal") && input.contains("newTimeframeWeeks") && input.contains("más práctica"), "{input}");
    assert_eq!(runner.week_calls(), 4 + 2);
    cleanup(&harness);
}

#[tokio::test]
async fn a_failed_adjustment_keeps_the_previous_plan_on_screen() {
    // Second outline never validates (3 weeks asked for 2 -> wrong count).
    let runner = PartsRunner::new(vec![Ok(Some(outline(4))), Ok(Some(outline(4))), Ok(Some(outline(4))), Ok(Some(outline(4))), Ok(Some(outline(4)))], always_valid());
    let harness = parts_harness(runner.clone());
    let mut session = ready_for_proposal(&harness, 4, 6.0).await;
    harness.service.run_followup_hand_off(None, &mut session).await.expect("first proposal");
    harness.service.store.save_roadmap_session(&session).expect("save");
    let sid = session.session_id.clone();

    let r = harness.service.adjust_proposed_plan(None, &sid, PlanAdjustment { timeframe_weeks: Some(2), ..Default::default() }).await.expect("a refused adjustment is not an error");

    assert_eq!(r.session.proposed_plan.as_ref().expect("never left without a plan").syllabus.milestones.len(), 4);
    assert_eq!(r.session.learner_profile_card.as_ref().unwrap().timeframe_weeks, 4, "the profile is restored too");
    cleanup(&harness);
}

#[test]
fn the_outline_check_covers_week_numbers_vague_text_and_jargon() {
    use super::super::grounding::build_profile_card;
    use super::super::plan_parts::outline_violations;
    let profile = build_profile_card(&full_assessment("Tema", 3, 6.0, EntryLevel::AbsoluteZero));
    assert!(outline_violations(&profile, &outline(3)).is_empty());

    let mut gap = outline(3);
    gap.weeks[2].week = 5;
    assert!(outline_violations(&profile, &gap).iter().any(|v| v.contains("consecutivos")));

    let mut vague = outline(3);
    vague.weeks[0].deliverable = "comprender la teoría".to_string();
    vague.weeks[1].weekly_goal = None;
    let v = outline_violations(&profile, &vague).join(" | ");
    assert!(v.contains("semana 1") && v.contains("es vago") && v.contains("semana 2") && v.contains("weeklyGoal"), "{v}");

    let mut jargon = outline(3);
    jargon.weeks[0].title = "Sprint 1".to_string();
    jargon.course_title = "socratic_prediction".to_string();
    assert!(outline_violations(&profile, &jargon).len() >= 2, "internal vocabulary never reaches the learner");

    let mut empty = outline(3);
    empty.core_focus.clear();
    empty.identified_needs.clear();
    empty.closing_question = "  ".to_string();
    assert_eq!(outline_violations(&profile, &empty).len(), 3);
}
