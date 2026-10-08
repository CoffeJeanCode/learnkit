use std::sync::Arc;

use crate::domain::learner_memory::{LearnerCognitiveMemory, RetrievalOutcome, LOCAL_LEARNER_ID};
use crate::domain::notebook::{BlockUpdate, ClassRecord, Course, DiagnosticBatteryState, DynamicBlockType};
use crate::domain::roadmap::RoadmapSession;
use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome, SkillEvidence};
use crate::error::{AppError, AppResult};
use crate::notebook_store::NotebookStore;
use crate::orchestration::Orchestrator;

/// Application service for the Notebook engine: bridges the confirmed
/// roadmap syllabus into the relational store, and drives the
/// `notebook_generator`/`notebook_gate_grader` execution turns (real
/// tool-calling — see `tools::notebook_tools`) that turn one class (one per
/// micromodule) into an iterative, mastery-gated notebook — ONE block at a
/// time, buffered in the background, never the whole class in one call. See
/// `generation` for the actual orchestration, `grading` for how a gate
/// submission is judged and how answer-key fields get stripped before a
/// block reaches the frontend, `grounding` for per-block content validation,
/// and `render` for the prompt/input strings sent to each agent.
pub struct NotebookService {
    store: NotebookStore,
    orchestrator: Arc<Orchestrator>,
}

impl NotebookService {
    pub fn new(store: NotebookStore, orchestrator: Arc<Orchestrator>) -> Self {
        Self { store, orchestrator }
    }

    /// Seeds `courses` / `syllabus_milestones` / `classes` from an
    /// already-SEALED roadmap session's confirmed syllabus — the bridge
    /// between the Roadmap & Syllabus Diagnostic Agent (an ephemeral JSON
    /// session) and this engine's relational store. One class per
    /// MICROMODULE (see `domain::roadmap::Micromodule`) via the shared
    /// `notebook_store::import_syllabus_into_store` helper — also used by
    /// `RoadmapService`'s seal-time import, so both paths stay identical.
    /// No notebook content is generated here for ANY class (including the
    /// first) — every class, first or not, gets its notebook lazily via
    /// `start_class_notebook` the first time the student actually opens it.
    pub fn import_course_from_roadmap(&self, session: &RoadmapSession) -> AppResult<Vec<ClassRecord>> {
        let package = session
            .roadmap_package
            .as_ref()
            .ok_or_else(|| AppError::InvalidInput("roadmap session has no sealed syllabus to import".to_string()))?;

        let (course_id, _, _) = crate::notebook_store::import_syllabus_into_store(
            &self.store,
            &package.syllabus,
            &package.learner_profile.target_goal,
        )?;
        self.store.list_classes_for_course(&course_id)
    }

    pub fn list_course_classes(&self, course_id: &str) -> AppResult<Vec<ClassRecord>> {
        self.store.list_classes_for_course(course_id)
    }

    /// Every course, newest first — startup routing: if any course has
    /// classes, the app lands on the classes view instead of the roadmap.
    pub fn list_courses(&self) -> AppResult<Vec<Course>> {
        self.store.list_courses()
    }

    /// Bulk overwrite of NON-GATING block content (e.g. the closing block's
    /// free-text reflection) — the one mutation path left that doesn't go
    /// through `submit_gate_response`'s grading/gating logic. Never touches
    /// `status`/`attempt_count`.
    pub fn save_notebook_state(&self, notebook_id: &str, blocks: &[BlockUpdate]) -> AppResult<()> {
        self.store.update_block_contents(notebook_id, blocks)
    }

    /// The course's diagnostic battery (goalAlignment + questions) plus
    /// whatever the student has answered so far — what the roadmap plan
    /// screen renders, `None` if this course has no battery on record.
    pub fn get_course_diagnostic_battery(&self, course_id: &str) -> AppResult<Option<DiagnosticBatteryState>> {
        self.store.get_course_diagnostic_battery(course_id)
    }

    pub fn save_diagnostic_battery_answers(&self, course_id: &str, answers: &std::collections::HashMap<String, String>) -> AppResult<()> {
        self.store.save_diagnostic_battery_answers(course_id, answers)
    }

    /// Records how the student SELF-reported one retrieval prompt (recalled it
    /// or not, after seeing the answer). Showing the block never touches
    /// memory — only this does, and only once per item (a repeat call is a
    /// no-op, so a double click can't inflate mastery). Self-reports are weak
    /// evidence: see `RetrievalOutcome::SelfRecalled`.
    pub fn record_retrieval_result(&self, block_id: &str, item_index: usize, recalled: bool) -> AppResult<()> {
        let block = self.store.get_block(block_id)?.ok_or_else(|| AppError::InvalidInput(format!("block not found: {block_id}")))?;
        if block.block_type != DynamicBlockType::SpacedInterleavedRetrieval {
            return Err(AppError::InvalidInput(format!("el bloque {block_id} no es un repaso espaciado")));
        }
        let mut content = block.content_json.clone();
        let item = content
            .get_mut("items")
            .and_then(|v| v.as_array_mut())
            .and_then(|items| items.get_mut(item_index))
            .ok_or_else(|| AppError::InvalidInput(format!("el repaso {block_id} no tiene el ítem {item_index}")))?;
        if item.get("reportedOutcome").is_some() {
            return Ok(());
        }
        let label = item.get("conceptLabel").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let outcome = if recalled { RetrievalOutcome::SelfRecalled } else { RetrievalOutcome::SelfForgot };
        item["reportedOutcome"] = serde_json::to_value(outcome).map_err(|e| AppError::Persistence(e.to_string()))?;
        self.store.update_block_contents(&block.document_id, &[BlockUpdate { id: block.id.clone(), content_json: content }])?;

        let mut memory = self.store.get_learner_memory(LOCAL_LEARNER_ID)?;
        let skill_id = memory.record_retrieval_outcome(&label, outcome, crate::notebook_store::now_ms());
        self.store.save_learner_memory(&memory)?;

        // The queued concept id IS the class (skill) the prompt reviews. The
        // evidence row is best-effort history; the memory update above is
        // what the student's scheduling depends on.
        if let Some(skill_id) = skill_id {
            if let Ok(Some(ctx)) = self.store.class_generation_context(&skill_id) {
                let evidence_outcome = if recalled { EvidenceOutcome::SelfRecalled } else { EvidenceOutcome::SelfForgot };
                let evidence = SkillEvidence::new(&skill_id, &ctx.course.id, EvidenceKind::Retrieval, evidence_outcome).with_block(block_id);
                if let Err(e) = self.store.append_skill_evidence(&evidence) {
                    tracing::warn!(error = %e, %skill_id, "failed to append retrieval evidence");
                }
            }
        }
        Ok(())
    }

    /// The append-only evidence history of ONE skill (class), oldest first —
    /// the source of truth any progress/achievement view must point at.
    pub fn get_skill_evidence(&self, skill_id: &str) -> AppResult<Vec<SkillEvidence>> {
        self.store.list_skill_evidence(skill_id)
    }

    /// Evidence for every skill of one course, oldest first.
    pub fn get_course_evidence(&self, course_id: &str) -> AppResult<Vec<SkillEvidence>> {
        self.store.list_course_evidence(course_id)
    }

    /// Read-only: the learner-memory viewer's data source. Always
    /// `LOCAL_LEARNER_ID` — see `domain::learner_memory`'s module doc for why
    /// this single-learner app has no other id to pass here.
    pub fn get_learner_memory(&self) -> AppResult<LearnerCognitiveMemory> {
        self.store.get_learner_memory(LOCAL_LEARNER_ID)
    }
}

mod events;
mod generation;
mod grading;
mod grounding;
mod render;

#[cfg(test)]
mod tests;
