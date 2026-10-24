//! The resource-research pipeline: once a course is sealed, find real, practical
//! resources (tutorials, docs, exercises) for each of its classes, so a class
//! about Linux, CAD or photography points at something to DO.
//!
//! ```text
//! planner (1 model call)  ->  web search (no model, parallel)  ->  curator (1 call per 8 classes)
//!   queries per class            real hits per class                picks among those hits
//!                                                                          |
//!                                                  ground_selection: a pick survives only if its URL
//!                                                  is a hit found for THAT class -> stored per class
//! ```
//!
//! It runs in the background after sealing and never blocks the student: with no
//! search key it is skipped (not an error), and a failure leaves the course
//! exactly as it was — classes simply have no resource list.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use serde::Serialize;

use crate::agents::research_curator_agent::RESEARCH_CURATOR_AGENT_ID;
use crate::agents::research_planner_agent::RESEARCH_PLANNER_AGENT_ID;
use crate::domain::calibration::CourseCalibration;
use crate::domain::notebook::ClassRecord;
use crate::domain::resources::{ResearchStatus, ResourceSelectionArgs, SearchHit, clean_hits, ground_selection, sanitize_plan};
use crate::error::{AppError, AppResult};
use crate::notebook_store::NotebookStore;
use crate::orchestration::Orchestrator;
use crate::prompt_encoding::toon_payload;
use crate::search::{SEARCH_KEY_ENV, SEARCH_KEY_ID, TavilySearch, WebSearch};
use crate::secrets::SecretVault;
use crate::tools::{ResearchCapture, ResearchStep, SharedResearchCapture};

/// Results requested per query: a few good ones, not a page of links.
const RESULTS_PER_QUERY: usize = 5;
/// Hits kept per class after merging its queries — bounds the curator's input.
const MAX_HITS_PER_CLASS: usize = 8;
/// Searches in flight at once.
const SEARCH_CONCURRENCY: usize = 4;
/// Classes the curator reviews per call.
const CLASSES_PER_CURATION: usize = 8;
/// Attempts per model call or search on a transient failure.
const MAX_ATTEMPTS: u8 = 2;

/// Pushed to the UI when a course's research changes state.
#[derive(Debug, Clone, Serialize)]
pub struct ResearchStatusEvent {
    pub course_id: String,
    pub status: ResearchStatus,
    pub detail: Option<String>,
}

pub struct ResearchService {
    orchestrator: Arc<Orchestrator>,
    store: NotebookStore,
    vault: Arc<dyn SecretVault>,
    /// Replaces the vault-key-built Tavily client — tests inject a fake.
    search_override: Option<Arc<dyn WebSearch>>,
    backoff: Duration,
}

impl ResearchService {
    pub fn new(orchestrator: Arc<Orchestrator>, store: NotebookStore, vault: Arc<dyn SecretVault>) -> Self {
        Self { orchestrator, store, vault, search_override: None, backoff: Duration::from_millis(500) }
    }

    #[cfg(test)]
    fn with_search(mut self, search: Arc<dyn WebSearch>) -> Self {
        self.search_override = Some(search);
        self.backoff = Duration::ZERO;
        self
    }

    /// The search client to use, or `None` when no key is configured. A key in
    /// the vault wins; the env var is a developer fallback.
    fn resolve_search(&self) -> Option<Arc<dyn WebSearch>> {
        if let Some(search) = &self.search_override {
            return Some(Arc::clone(search));
        }
        let key = self
            .vault
            .get_provider_key(SEARCH_KEY_ID)
            .ok()
            .flatten()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| std::env::var(SEARCH_KEY_ENV).ok().filter(|k| !k.trim().is_empty()))?;
        Some(Arc::new(TavilySearch::new(key.trim().to_string())))
    }

    /// Starts the research of `course_id` in the background. Cheap and safe to
    /// call from anywhere, any number of times: the claim inside
    /// [`run_for_course`](Self::run_for_course) makes a repeat a no-op while it
    /// runs or once it is done.
    pub fn start(self: &Arc<Self>, app: Option<tauri::AppHandle>, course_id: String) {
        let service = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            if let Err(e) = service.run_for_course(app.as_ref(), &course_id).await {
                tracing::warn!(course_id = %course_id, error = %e, "resource research could not record its outcome");
            }
        });
    }

    pub fn status(&self, course_id: &str) -> AppResult<Option<(ResearchStatus, Option<String>)>> {
        self.store.get_research_status(course_id)
    }

    /// Runs the whole pipeline for one course and records how it ended.
    /// Returns the final status (`Running` when another run already holds it).
    pub async fn run_for_course(&self, app: Option<&tauri::AppHandle>, course_id: &str) -> AppResult<ResearchStatus> {
        if !self.store.try_begin_research(course_id)? {
            return Ok(self.store.get_research_status(course_id)?.map(|(s, _)| s).unwrap_or(ResearchStatus::Running));
        }
        self.emit(app, course_id, ResearchStatus::Running, None);
        let (status, detail) = match self.run_pipeline(app, course_id).await {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!(course_id = %course_id, error = %e, "resource research failed");
                (ResearchStatus::Failed, Some(e.to_string().chars().take(300).collect()))
            }
        };
        self.store.set_research_status(course_id, status, detail.as_deref())?;
        self.emit(app, course_id, status, detail);
        Ok(status)
    }

    fn emit(&self, app: Option<&tauri::AppHandle>, course_id: &str, status: ResearchStatus, detail: Option<String>) {
        if let Some(app) = app {
            use tauri::Emitter;
            let _ = app.emit("research://status", &ResearchStatusEvent { course_id: course_id.to_string(), status, detail });
        }
    }

    async fn run_pipeline(&self, app: Option<&tauri::AppHandle>, course_id: &str) -> AppResult<(ResearchStatus, Option<String>)> {
        let Some(search) = self.resolve_search() else {
            return Ok((ResearchStatus::SkippedNoKey, None));
        };
        let ctx = self.load_course(course_id)?;
        if ctx.classes.is_empty() {
            return Ok((ResearchStatus::Done, Some("el curso no tiene clases".to_string())));
        }

        // 1. Plan: queries per class.
        let plan_capture = self.run_step(app, RESEARCH_PLANNER_AGENT_ID, ResearchStep::Plan, &render_planner_input(&ctx)).await?;
        let plan = plan_capture.plan.ok_or_else(|| AppError::AgentExecutionFailed("el planificador no entregó consultas".to_string()))?;
        let known: HashSet<String> = ctx.classes.iter().map(|c| c.record.id.clone()).collect();
        let (queries, dropped) = sanitize_plan(&plan, &known);
        if !dropped.is_empty() {
            tracing::info!(course_id = %course_id, ?dropped, "research plan entries dropped");
        }
        if queries.is_empty() {
            return Ok((ResearchStatus::Done, Some("sin consultas útiles".to_string())));
        }

        // 2. Search (no model): hits per class, merged and capped.
        let hits_by_class = self.search_all(&search, &queries).await?;

        // 3. Curate in batches; ground every pick against the real hits.
        let with_hits: Vec<&ClassView> = ctx.classes.iter().filter(|c| hits_by_class.contains_key(&c.record.id)).collect();
        let (mut classes_with_resources, mut failed_batches) = (0usize, 0usize);
        for batch in with_hits.chunks(CLASSES_PER_CURATION) {
            let input = render_curator_input(&ctx, batch, &hits_by_class);
            let selection = match self.run_step(app, RESEARCH_CURATOR_AGENT_ID, ResearchStep::Select, &input).await {
                Ok(capture) => capture.selection,
                Err(e) => {
                    tracing::warn!(course_id = %course_id, error = %e, "curation batch failed");
                    None
                }
            };
            let Some(selection) = selection else {
                failed_batches += 1;
                continue;
            };
            classes_with_resources += self.store_batch(batch, &selection, &hits_by_class)?;
        }

        if classes_with_resources == 0 && failed_batches > 0 {
            return Err(AppError::AgentExecutionFailed("no se pudieron curar recursos".to_string()));
        }
        Ok((ResearchStatus::Done, Some(format!("{classes_with_resources} clases con recursos"))))
    }

    /// Grounds one batch's picks and stores them, clearing any previous list of
    /// a class the curator left empty. Returns how many classes now have resources.
    fn store_batch(&self, batch: &[&ClassView], selection: &ResourceSelectionArgs, hits_by_class: &HashMap<String, Vec<SearchHit>>) -> AppResult<usize> {
        let mut grounded = ground_selection(selection, hits_by_class);
        let mut with_resources = 0;
        for class in batch {
            let resources = grounded.remove(&class.record.id).unwrap_or_default();
            if !resources.is_empty() {
                with_resources += 1;
            }
            self.store.set_class_resources(&class.record.id, &resources)?;
        }
        Ok(with_resources)
    }

    fn load_course(&self, course_id: &str) -> AppResult<CourseView> {
        let course = self
            .store
            .list_courses()?
            .into_iter()
            .find(|c| c.id == course_id)
            .ok_or_else(|| AppError::InvalidInput(format!("course not found: {course_id}")))?;
        let weeks: HashMap<String, (u16, String)> =
            self.store.list_milestones_for_course(course_id)?.into_iter().map(|(id, week, title)| (id, (week, title))).collect();
        let classes = self
            .store
            .list_classes_for_course(course_id)?
            .into_iter()
            .map(|record| {
                let (week, week_title) = weeks.get(&record.milestone_id).cloned().unwrap_or((0, String::new()));
                ClassView { record, week, week_title }
            })
            .collect();
        Ok(CourseView { title: course.title, goal: course.target_goal, calibration: self.store.get_course_calibration(course_id)?, classes })
    }

    /// One model call with a retry on transient failure. Returns the capture
    /// even when the step produced nothing (the caller decides what that means).
    async fn run_step(&self, app: Option<&tauri::AppHandle>, agent_id: &str, step: ResearchStep, input: &str) -> AppResult<ResearchCapture> {
        let mut last_err = None;
        for attempt in 0..MAX_ATTEMPTS {
            let capture: SharedResearchCapture = Arc::new(Mutex::new(ResearchCapture::default()));
            match self.orchestrator.run_research_agent(app, agent_id, input, capture.clone(), step).await {
                Ok(_) => return Ok(capture.lock().unwrap_or_else(|e| e.into_inner()).clone()),
                Err(e) if e.is_transient() && attempt + 1 < MAX_ATTEMPTS => {
                    tracing::warn!(agent_id, error = %e, attempt, "research step failed, retrying");
                    last_err = Some(e);
                    tokio::time::sleep(self.backoff).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.expect("loop only exits through an Err branch"))
    }

    /// Runs every query (bounded concurrency) and merges the hits per class.
    /// A rejected key aborts the run (it cannot recover); a failure of a single
    /// query is tolerated; losing ALL of them is an error.
    async fn search_all(&self, search: &Arc<dyn WebSearch>, queries: &[crate::domain::resources::ResearchQuery]) -> AppResult<HashMap<String, Vec<SearchHit>>> {
        let backoff = self.backoff;
        let results: Vec<(String, AppResult<Vec<SearchHit>>)> = futures::stream::iter(queries.iter().cloned())
            .map(|q| {
                let search = Arc::clone(search);
                async move {
                    let mut outcome = search.search(&q.query, RESULTS_PER_QUERY).await;
                    if matches!(&outcome, Err(e) if e.is_transient()) {
                        tokio::time::sleep(backoff).await;
                        outcome = search.search(&q.query, RESULTS_PER_QUERY).await;
                    }
                    (q.class_id, outcome)
                }
            })
            .buffer_unordered(SEARCH_CONCURRENCY)
            .collect()
            .await;

        let mut merged: HashMap<String, Vec<SearchHit>> = HashMap::new();
        let mut first_error: Option<AppError> = None;
        let mut succeeded = 0usize;
        for (class_id, outcome) in results {
            match outcome {
                Ok(hits) => {
                    succeeded += 1;
                    merged.entry(class_id).or_default().extend(hits);
                }
                Err(e) if !e.is_transient() => return Err(e),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        if succeeded == 0 {
            return Err(first_error.unwrap_or_else(|| AppError::ProviderConnectionFailed("el buscador no devolvió resultados".to_string())));
        }
        // Dedupe across a class's queries, keep the provider's order, cap, and
        // drop classes that found nothing.
        Ok(merged
            .into_iter()
            .filter_map(|(class_id, hits)| {
                let mut cleaned = clean_hits(hits);
                cleaned.truncate(MAX_HITS_PER_CLASS);
                (!cleaned.is_empty()).then_some((class_id, cleaned))
            })
            .collect())
    }
}

struct ClassView {
    record: ClassRecord,
    week: u16,
    week_title: String,
}

struct CourseView {
    title: String,
    goal: String,
    calibration: Option<CourseCalibration>,
    classes: Vec<ClassView>,
}

/// The learner facts the research needs, from what the course stored.
fn learner_json(ctx: &CourseView) -> serde_json::Value {
    let mut profile = serde_json::json!({ "topic": ctx.title, "goal": ctx.goal });
    if let (Some(cal), Some(obj)) = (&ctx.calibration, profile.as_object_mut()) {
        obj.insert("declaredLevel".to_string(), serde_json::json!(cal.declared_level));
        obj.insert("weeklyHours".to_string(), serde_json::json!(cal.weekly_commitment_hours));
    }
    profile
}

fn learner_context_json(ctx: &CourseView) -> serde_json::Value {
    ctx.calibration.as_ref().and_then(|c| c.learner_context.as_ref()).map(|c| c.describe()).unwrap_or(serde_json::Value::Null)
}

fn render_planner_input(ctx: &CourseView) -> String {
    let classes: Vec<serde_json::Value> = ctx
        .classes
        .iter()
        .map(|c| serde_json::json!({ "classId": c.record.id, "week": c.week, "weekTitle": c.week_title, "title": c.record.title, "objective": c.record.objective }))
        .collect();
    let context = serde_json::json!({ "learnerProfile": learner_json(ctx), "learnerContext": learner_context_json(ctx), "classes": classes });
    format!("Propón las consultas de búsqueda llamando a submit_research_plan según tu system prompt.\n\n{}", toon_payload(&context))
}

fn render_curator_input(ctx: &CourseView, batch: &[&ClassView], hits_by_class: &HashMap<String, Vec<SearchHit>>) -> String {
    let classes: Vec<serde_json::Value> = batch
        .iter()
        .map(|c| {
            let results: Vec<serde_json::Value> = hits_by_class
                .get(&c.record.id)
                .map(|hits| hits.iter().map(|h| serde_json::json!({ "url": h.url, "title": h.title, "snippet": h.snippet })).collect())
                .unwrap_or_default();
            serde_json::json!({ "classId": c.record.id, "title": c.record.title, "objective": c.record.objective, "results": results })
        })
        .collect();
    let context = serde_json::json!({ "learnerProfile": learner_json(ctx), "learnerContext": learner_context_json(ctx), "classes": classes });
    format!("Elige los recursos llamando a submit_resource_selection según tu system prompt.\n\n{}", toon_payload(&context))
}

#[cfg(test)]
mod tests;
