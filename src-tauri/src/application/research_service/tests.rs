//! The pipeline end to end with a scripted model and a fake search. What these
//! pin: invented links never get stored, no key means nothing is attempted, a
//! finished research is never repeated, and failures leave the course untouched.

use std::collections::VecDeque;

use async_trait::async_trait;

use super::*;
use crate::agents::AgentRegistry;
use crate::domain::learning_context::{ContextAnswer, LearningContext, fallback_question_set};
use crate::domain::resources::{ClassPicks, ResearchPlanArgs, ResearchQuery, ResourceKind, ResourcePick};
use crate::domain::roadmap::{
    CapstoneProject, Deliverable, DeliverableArtifactType, EntryLevel, Micromodule, Milestone, RoadmapSyllabusPackage,
};
use crate::orchestration::PromptRunner;
use crate::providers::factory::PromptOutput;
use crate::secrets::MemoryVault;

// --- Scripted model ----------------------------------------------------------

#[derive(Default)]
struct ScriptedRunner {
    plans: Mutex<VecDeque<AppResult<Option<ResearchPlanArgs>>>>,
    selections: Mutex<VecDeque<AppResult<Option<ResourceSelectionArgs>>>>,
    inputs: Mutex<Vec<(ResearchStep, String)>>,
}

impl ScriptedRunner {
    fn calls(&self, step: ResearchStep) -> usize {
        self.inputs.lock().unwrap().iter().filter(|(s, _)| *s == step).count()
    }
    fn total_calls(&self) -> usize {
        self.inputs.lock().unwrap().len()
    }
}

#[async_trait]
impl PromptRunner for ScriptedRunner {
    async fn run(&self, _: &str, _: &str, _: &str, _: &str, _: &[String]) -> AppResult<PromptOutput> {
        unreachable!("research only drives run_research_execution")
    }

    async fn run_research_execution(
        &self,
        _provider_id: &str,
        _model: &str,
        _system_prompt: &str,
        input: &str,
        capture: SharedResearchCapture,
        step: ResearchStep,
    ) -> AppResult<PromptOutput> {
        self.inputs.lock().unwrap().push((step, input.to_string()));
        match step {
            ResearchStep::Plan => match self.plans.lock().unwrap().pop_front().expect("scripted plan available")? {
                Some(plan) => capture.lock().unwrap().plan = Some(plan),
                None => {}
            },
            ResearchStep::Select => match self.selections.lock().unwrap().pop_front().expect("scripted selection available")? {
                Some(selection) => capture.lock().unwrap().selection = Some(selection),
                None => {}
            },
            ResearchStep::Questions | ResearchStep::Outline | ResearchStep::Week => unreachable!("the research pipeline only plans and selects"),
        }
        Ok(PromptOutput { text: String::new(), tool_calls: vec![] })
    }
}

// --- Fake search -------------------------------------------------------------

#[derive(Clone)]
enum Reply {
    Hits(Vec<SearchHit>),
    Permanent,
    Transient,
    /// Fails once, then answers.
    TransientThenHits(Vec<SearchHit>),
}

#[derive(Default)]
struct FakeSearch {
    replies: Mutex<HashMap<String, Reply>>,
    seen: Mutex<HashMap<String, usize>>,
}

impl FakeSearch {
    fn total_searches(&self) -> usize {
        self.seen.lock().unwrap().values().sum()
    }
}

#[async_trait]
impl WebSearch for FakeSearch {
    async fn search(&self, query: &str, _max_results: usize) -> AppResult<Vec<SearchHit>> {
        let nth = {
            let mut seen = self.seen.lock().unwrap();
            let n = seen.entry(query.to_string()).or_insert(0);
            *n += 1;
            *n
        };
        match self.replies.lock().unwrap().get(query).cloned().unwrap_or(Reply::Hits(vec![])) {
            Reply::Hits(h) => Ok(h),
            Reply::Permanent => Err(AppError::ProviderKeyMissing("buscador".to_string())),
            Reply::Transient => Err(AppError::ProviderConnectionFailed("caído".to_string())),
            Reply::TransientThenHits(h) => {
                if nth == 1 { Err(AppError::ProviderConnectionFailed("parpadeo".to_string())) } else { Ok(h) }
            }
        }
    }
}

// --- Fixtures ----------------------------------------------------------------

fn hit(title: &str, url: &str) -> SearchHit {
    SearchHit { title: title.to_string(), url: url.to_string(), snippet: format!("resumen de {title}") }
}

fn syllabus(weeks: u16, sessions_per_week: usize) -> RoadmapSyllabusPackage {
    RoadmapSyllabusPackage {
        course_title: "Fotografía".to_string(),
        total_weeks: weeks,
        pace_hours_per_week: 3.0,
        milestones: (1..=weeks)
            .map(|week| Milestone {
                week,
                title: format!("Semana {week}"),
                deliverable: "Entregable".to_string(),
                weekly_goal: Some("Avanzar".to_string()),
                micromodules: (0..sessions_per_week)
                    .map(|i| Micromodule {
                        label: format!("Sesión {}", i + 1),
                        hours: 1.0,
                        focus: Some("Foco".to_string()),
                        deliverable: Deliverable { artifact_type: DeliverableArtifactType::TestsPassing, description: "Artefacto".to_string() },
                        objective: Some(format!("Objetivo {week}.{i}")),
                        interactive_blocks: vec!["hands_on_mission".to_string()],
                    })
                    .collect(),
            })
            .collect(),
        capstone_project: CapstoneProject { title: "P".to_string(), description: "D".to_string(), verifiable_evidence: "E".to_string() },
    }
}

struct Fixture {
    service: ResearchService,
    runner: Arc<ScriptedRunner>,
    store: NotebookStore,
    course_id: String,
    class_ids: Vec<String>,
}

/// A sealed-looking course (weeks x sessions classes), with a photography
/// context stored on its calibration — like `seal_session` leaves it.
fn fixture(weeks: u16, sessions: usize, search: Option<FakeSearch>) -> (Fixture, Option<Arc<FakeSearch>>) {
    let store = NotebookStore::open_in_memory().expect("store");
    let (course_id, _, _) = crate::notebook_store::import_syllabus_into_store(&store, &syllabus(weeks, sessions), "Fotos en manual").expect("import");
    let mut calibration = crate::domain::calibration::calibrate(EntryLevel::AbsoluteZero, 3.0, None);
    calibration.learner_context = Some(
        LearningContext::validate(
            &fallback_question_set("Fotografía"),
            &[ContextAnswer { question_id: "prior".into(), answer: "Nada, empiezo de cero".into() }, ContextAnswer { question_id: "tools".into(), answer: "Un celular".into() }],
        )
        .expect("valid context"),
    );
    store.set_course_calibration(&course_id, &calibration).expect("calibration");
    let class_ids: Vec<String> = store.list_classes_for_course(&course_id).expect("classes").into_iter().map(|c| c.id).collect();

    let agents = Arc::new(AgentRegistry::new());
    agents.register(crate::agents::research_planner_agent::definition()).expect("planner");
    agents.register(crate::agents::research_curator_agent::definition()).expect("curator");
    let runner = Arc::new(ScriptedRunner::default());
    let orchestrator = Arc::new(Orchestrator::new(agents, runner.clone()));
    let mut service = ResearchService::new(orchestrator, store.clone(), Arc::new(MemoryVault::new()));
    let search = search.map(Arc::new);
    if let Some(s) = &search {
        service = service.with_search(s.clone());
    }
    (Fixture { service, runner, store, course_id, class_ids }, search)
}

fn plan_for(class_ids: &[String], queries_each: &[&str]) -> ResearchPlanArgs {
    ResearchPlanArgs {
        queries: class_ids.iter().flat_map(|id| queries_each.iter().map(move |q| ResearchQuery { class_id: id.clone(), query: format!("{q} {id}") })).collect(),
    }
}

fn pick(url: &str) -> ResourcePick {
    ResourcePick { url: url.to_string(), kind: ResourceKind::Tutorial, why: "sirve".to_string(), what_to_do: "empieza aquí".to_string() }
}

// --- Tests -------------------------------------------------------------------

#[tokio::test]
async fn the_pipeline_stores_only_resources_that_the_search_really_returned() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    let query = format!("modo manual {class}");
    search.as_ref().unwrap().replies.lock().unwrap().insert(query, Reply::Hits(vec![hit("Guía real del modo manual", "https://real.dev/manual"), hit("Otra", "https://real.dev/otra")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs {
        classes: vec![ClassPicks { class_id: class.clone(), picks: vec![pick("https://invented.example/tutorial-perfecto"), pick("https://real.dev/manual")] }],
    })));

    let status = fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    assert_eq!(status, ResearchStatus::Done);
    let stored = fx.store.get_class_resources(&class).expect("read");
    assert_eq!(stored.len(), 1, "the invented link was dropped before storage");
    assert_eq!((stored[0].url.as_str(), stored[0].title.as_str()), ("https://real.dev/manual", "Guía real del modo manual"), "title from the hit");
    assert_eq!(fx.store.get_research_status(&fx.course_id).expect("status"), Some((ResearchStatus::Done, Some("1 clases con recursos".to_string()))));
    assert_eq!((fx.runner.calls(ResearchStep::Plan), fx.runner.calls(ResearchStep::Select)), (1, 1), "one planner call and one curator call for a small course");
}

#[tokio::test]
async fn without_a_search_key_nothing_is_attempted() {
    // No override and an empty vault. (The env fallback is a developer
    // convenience; this test must not depend on it being unset, so it only
    // runs the branch when the variable is absent.)
    if std::env::var(SEARCH_KEY_ENV).is_ok() {
        return;
    }
    let (fx, _) = fixture(1, 1, None);

    let status = fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    assert_eq!(status, ResearchStatus::SkippedNoKey);
    assert_eq!(fx.runner.total_calls(), 0, "no model call is spent without a way to search");
    assert!(fx.store.get_class_resources(&fx.class_ids[0]).expect("read").is_empty());
}

#[tokio::test]
async fn a_key_added_later_lets_a_skipped_research_run() {
    if std::env::var(SEARCH_KEY_ENV).is_ok() {
        return;
    }
    let (fx, _) = fixture(1, 1, None);
    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("run"), ResearchStatus::SkippedNoKey);

    fx.service.vault.save_provider_key(SEARCH_KEY_ID, "tvly-key").expect("save key");
    assert!(fx.service.resolve_search().is_some(), "the vault key is picked up on the next run");
    assert!(fx.store.try_begin_research(&fx.course_id).expect("claim"), "a skip is not final");
}

#[tokio::test]
async fn a_finished_research_is_never_repeated() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Hits(vec![hit("A", "https://a.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs { classes: vec![ClassPicks { class_id: class, picks: vec![pick("https://a.dev")] }] })));

    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("first"), ResearchStatus::Done);
    let before = (fx.runner.total_calls(), search.as_ref().unwrap().total_searches());

    // Every sealed turn asks to start it; it must cost nothing once done.
    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("second"), ResearchStatus::Done);
    assert_eq!((fx.runner.total_calls(), search.as_ref().unwrap().total_searches()), before, "no model call and no search the second time");
}

#[tokio::test]
async fn a_rejected_search_key_fails_the_research_without_spending_the_curator() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Permanent);
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));

    let status = fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    assert_eq!(status, ResearchStatus::Failed);
    assert_eq!(fx.runner.calls(ResearchStep::Select), 0, "nothing to curate when the key is refused");
    let (_, detail) = fx.store.get_research_status(&fx.course_id).expect("status").expect("row");
    assert!(detail.is_some_and(|d| d.contains("buscador")), "the reason is recorded");
}

#[tokio::test]
async fn one_transient_search_failure_is_retried_and_does_not_sink_the_research() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::TransientThenHits(vec![hit("A", "https://a.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs { classes: vec![ClassPicks { class_id: class.clone(), picks: vec![pick("https://a.dev")] }] })));

    let status = fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    assert_eq!(status, ResearchStatus::Done);
    assert_eq!(search.unwrap().total_searches(), 2, "failed once, then succeeded on the retry");
    assert_eq!(fx.store.get_class_resources(&class).expect("read").len(), 1);
}

#[tokio::test]
async fn a_planner_that_delivers_nothing_fails_and_the_course_can_try_again() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    fx.runner.plans.lock().unwrap().push_back(Ok(None));

    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("run"), ResearchStatus::Failed);
    assert_eq!(search.as_ref().unwrap().total_searches(), 0);

    // A later attempt (the user retries, or the app restarts) may run again.
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Hits(vec![hit("A", "https://a.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs { classes: vec![ClassPicks { class_id: class, picks: vec![pick("https://a.dev")] }] })));
    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("retry"), ResearchStatus::Done);
}

#[tokio::test]
async fn a_curator_that_only_invents_links_stores_nothing_but_still_finishes() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Hits(vec![hit("Real", "https://real.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs {
        classes: vec![ClassPicks { class_id: class.clone(), picks: vec![pick("https://made-up.example/a"), pick("https://made-up.example/b")] }],
    })));

    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("run"), ResearchStatus::Done);
    assert!(fx.store.get_class_resources(&class).expect("read").is_empty(), "nothing invented is ever shown");
}

#[tokio::test]
async fn a_big_course_is_curated_in_batches_of_eight_classes() {
    // 4 weeks x 3 sessions = 12 classes -> batches of 8 + 4.
    let (fx, search) = fixture(4, 3, Some(FakeSearch::default()));
    assert_eq!(fx.class_ids.len(), 12);
    for id in &fx.class_ids {
        search.as_ref().unwrap().replies.lock().unwrap().insert(format!("tutorial {id}"), Reply::Hits(vec![hit(&format!("Guía {id}"), &format!("https://real.dev/{id}"))]));
    }
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["tutorial"]))));
    for batch in fx.class_ids.chunks(8) {
        fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs {
            classes: batch.iter().map(|id| ClassPicks { class_id: id.clone(), picks: vec![pick(&format!("https://real.dev/{id}"))] }).collect(),
        })));
    }

    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("run"), ResearchStatus::Done);

    assert_eq!(fx.runner.calls(ResearchStep::Select), 2, "12 classes -> two curator calls, not twelve");
    assert!(fx.class_ids.iter().all(|id| fx.store.get_class_resources(id).expect("read").len() == 1), "every class got its own resource");
}

#[tokio::test]
async fn the_model_inputs_carry_the_learners_equipment_and_the_real_class_ids() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Hits(vec![hit("A", "https://a.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs::default())));

    fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    let inputs = fx.runner.inputs.lock().unwrap().clone();
    let planner = &inputs.iter().find(|(s, _)| *s == ResearchStep::Plan).expect("planner input").1;
    assert!(planner.contains(&class) && planner.contains("Un celular") && planner.contains("¿Con qué herramientas o material cuentas para practicar?"), "{planner}");
    assert!(planner.contains("absolute_zero"), "the declared level reaches the planner");
    let curator = &inputs.iter().find(|(s, _)| *s == ResearchStep::Select).expect("curator input").1;
    assert!(curator.contains("https://a.dev") && curator.contains(&class), "the curator sees the REAL hits it may choose from: {curator}");
}

#[tokio::test]
async fn an_empty_search_for_every_class_ends_cleanly_with_no_curation() {
    let (fx, _) = fixture(1, 1, Some(FakeSearch::default()));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["algo sin resultados"]))));

    assert_eq!(fx.service.run_for_course(None, &fx.course_id).await.expect("run"), ResearchStatus::Done);
    assert_eq!(fx.runner.calls(ResearchStep::Select), 0, "no hits, nothing to curate");
}

#[tokio::test]
async fn concurrent_triggers_run_the_pipeline_once() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Hits(vec![hit("A", "https://a.dev")]));
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));
    fx.runner.selections.lock().unwrap().push_back(Ok(Some(ResourceSelectionArgs { classes: vec![ClassPicks { class_id: class, picks: vec![pick("https://a.dev")] }] })));

    let (a, b) = tokio::join!(fx.service.run_for_course(None, &fx.course_id), fx.service.run_for_course(None, &fx.course_id));

    assert!(a.is_ok() && b.is_ok());
    assert_eq!(fx.runner.calls(ResearchStep::Plan), 1, "the second trigger found it already claimed");
}

#[tokio::test]
async fn losing_every_search_is_a_failure_not_an_empty_success() {
    let (fx, search) = fixture(1, 1, Some(FakeSearch::default()));
    let class = fx.class_ids[0].clone();
    search.as_ref().unwrap().replies.lock().unwrap().insert(format!("modo manual {class}"), Reply::Transient);
    fx.runner.plans.lock().unwrap().push_back(Ok(Some(plan_for(&fx.class_ids, &["modo manual"]))));

    let status = fx.service.run_for_course(None, &fx.course_id).await.expect("run");

    assert_eq!(status, ResearchStatus::Failed, "an outage must not be recorded as `done` with no resources — a retry has to stay possible");
    assert_eq!(search.unwrap().total_searches(), 2, "each query was retried once before giving up");
    assert!(fx.store.try_begin_research(&fx.course_id).expect("claim"), "a failed research can be started again");
}
