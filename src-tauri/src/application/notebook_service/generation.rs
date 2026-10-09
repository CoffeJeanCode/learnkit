//! Per-block generation and gating orchestration — the core of the
//! redesign. Replaces the retired `generate_class_notebook` (one call, whole
//! notebook) with `start_class_notebook` (returns block 1 fast, buffers
//! block 2 in the background) and `submit_gate_response` (the literal gate:
//! grades a submission, advances the cursor on a pass, escalates on a 3rd
//! fail). Mirrors `roadmap_service::flow`'s retry/grounding-retry shape, one
//! block at a time instead of one whole payload at a time.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::AppHandle;

use crate::agents::notebook_agent::NOTEBOOK_AGENT_ID;
use crate::agents::pedagogical_critic_agent::PEDAGOGICAL_CRITIC_AGENT_ID;
use crate::domain::learner_memory::{LOCAL_LEARNER_ID, iso_date_from_ms};
use crate::domain::notebook::{
    BlockStatus, BlockUpdate, ClassRecord, ClosureFeedback, DynamicBlockType, GateResult, GateSubmission, GeneratedSectionBlock,
    NotebookBlock, NotebookPayload,
};
use crate::domain::pedagogy_guardrails::{block_guardrail_violations, needs_llm_critic};
use crate::domain::scaffolding;
use crate::domain::study::StudyEventKind;
use crate::domain::skill_status::MIN_RETENTION_INTERVAL_MS;
use crate::domain::skill_evidence::{EvidenceKind, EvidenceOutcome, RubricCriterion, SkillEvidence};
use crate::error::{AppError, AppResult};
use crate::notebook_store::{ClassGenerationContext, NotebookStore};
use crate::orchestration::Orchestrator;
use crate::orchestration::loop_runner::{Cause, CriticVerdict, GeneratorCriticLoop, run_generator_critic_loop};
use crate::tools::{BlockAuditCapture, NotebookBlockCapture};

use super::grading;
use super::grounding::{self, MasteryProgress, dominance_violation, mastery_progress, single_block_violations};
use super::render;
use super::NotebookService;

/// Same reasoning as the retired whole-notebook path: the failures actually
/// observed are near-instant connection errors, not slow timeouts, so a
/// short backoff (not a long one) is what actually helps.
const MAX_ATTEMPTS: u8 = 3;
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Failed submissions on the SAME gate before escalating to a re-approach
/// block instead of another scaffold hint — confirmed product decision.
const MAX_GATE_ATTEMPTS: u32 = 3;

impl NotebookService {
    /// Sequential-path guard: a class can only be opened once EVERY class
    /// before it in its course is complete ("understood and practiced" —
    /// see [`crate::domain::notebook::class_is_complete`]). Applies to both
    /// the fresh-generation and the resume path of `start_class_notebook`,
    /// so the next class can't be reached by any client — the visual lock
    /// on the path is only its mirror. The class's OWN completion is never
    /// checked here: you must be able to open the class you're working on.
    fn ensure_prior_classes_complete(&self, target: &ClassRecord, course_id: &str) -> AppResult<()> {
        let classes = self.store.list_classes_for_course(course_id)?;
        for prior in classes.iter().filter(|c| c.order_index < target.order_index) {
            if !prior.complete {
                return Err(AppError::InvalidInput(format!(
                    "Termina primero \"{}\" (practica todos sus ejercicios y escribe el cierre) para desbloquear \"{}\".",
                    prior.title, target.title
                )));
            }
        }
        Ok(())
    }

    /// Returns FAST: block 1 only (generated synchronously — nothing to
    /// buffer yet). If block 1 isn't a gate and isn't the closing block,
    /// spawns block 2's generation in the background so it's ready by the
    /// time the student finishes reading. Resuming an already-started
    /// notebook (any blocks already persisted) just returns current
    /// progress — it never regenerates block 1.
    pub async fn start_class_notebook(&self, app: Option<&AppHandle>, class_id: &str) -> AppResult<NotebookPayload> {
        let ctx = self
            .store
            .class_generation_context(class_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("class not found: {class_id}")))?;
        self.ensure_prior_classes_complete(&ctx.class, &ctx.milestone.course_id)?;
        self.log_study_event(StudyEventKind::ClassOpened, Some(class_id));
        let doc = self.store.ensure_document_shell(class_id, &ctx.class.title)?;

        if let Some(existing) = self.load_reconciled(class_id)? {
            if !existing.blocks.is_empty() {
                return Ok(NotebookPayload { document: existing.document, blocks: grading::redact_all(existing.blocks) });
            }
        }

        let Some(block) = generate_and_persist_block(&self.store, &self.orchestrator, app, class_id, &doc.id, None).await? else {
            // Another concurrent starter (StrictMode's double-mount fires
            // two invokes; the loser's candidate was discarded by the
            // count-guarded insert) — hand back the WINNER's progress.
            let winner = self
                .load_reconciled(class_id)?
                .ok_or_else(|| AppError::Persistence(format!("notebook concurrently created for {class_id}")))?;
            return Ok(NotebookPayload { document: winner.document, blocks: grading::redact_all(winner.blocks) });
        };
        self.store.advance_cursor_to_at_least(&doc.id, cursor_target_after_insert(block.order_index, block.block_type))?;

        if should_auto_chain(&block) {
            self.spawn_background_next_block(app, class_id.to_string(), doc.id.clone());
        }

        let refreshed = self.store.get_document(&doc.id)?.unwrap_or(doc);
        Ok(NotebookPayload { document: refreshed, blocks: vec![grading::redact_block(&block)] })
    }

    /// Wipes an atypical/broken class notebook so the student can start it
    /// over from scratch — the whole-class counterpart to
    /// [`Self::regenerate_block`]'s single-block in-place repair, for cases
    /// a targeted block swap can't fix (a bad overall composition, a stuck
    /// mastery loop, several corrupted blocks at once). Pure deletion: the
    /// caller (the frontend's confirm button) triggers a fresh
    /// `start_class_notebook` afterward, same as opening the class for the
    /// first time — this method does NOT regenerate anything itself, so a
    /// wipe that's never followed up simply leaves the class notebook-less
    /// (same as before it was ever opened) rather than silently starting a
    /// background generation the caller didn't ask for.
    pub fn reset_class_notebook(&self, class_id: &str) -> AppResult<()> {
        self.store
            .class_generation_context(class_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("class not found: {class_id}")))?;
        self.store.delete_notebook_for_class(class_id)
    }

    /// Whatever's persisted so far for this class — the resume/reload path.
    /// Never triggers generation itself (that only ever happens from
    /// `start_class_notebook`'s first call or a gate resolving).
    pub fn get_class_notebook_progress(&self, class_id: &str) -> AppResult<NotebookPayload> {
        let payload = self
            .load_reconciled(class_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("no notebook started yet for class: {class_id}")))?;
        Ok(NotebookPayload { document: payload.document, blocks: grading::redact_all(payload.blocks) })
    }

    /// Loads the class's raw payload with its cursor RECONCILED against the
    /// persisted blocks first (see [`reconciled_cursor`]) — the self-heal
    /// for a document whose cursor fell behind a lost concurrent advance,
    /// which otherwise rejects every later gate as "not the active block"
    /// and wedges the notebook. Monotonic: a healthy cursor is untouched.
    fn load_reconciled(&self, class_id: &str) -> AppResult<Option<NotebookPayload>> {
        let Some(mut payload) = self.store.load_notebook_by_class(class_id)? else {
            return Ok(None);
        };
        if payload.blocks.is_empty() {
            return Ok(Some(payload));
        }
        let target = reconciled_cursor(&payload.blocks);
        if target > payload.document.current_block_index {
            self.store.advance_cursor_to_at_least(&payload.document.id, target)?;
            payload.document.current_block_index = target;
        }
        Ok(Some(payload))
    }

    /// The literal mastery gate. Rejects if `block_id` isn't the document's
    /// currently active block (stale/out-of-order submit — mirrors
    /// `RoadmapService::answer_diagnostic_question`'s stale-battery-answer
    /// rejection) or if the block was already resolved. On a pass, advances
    /// the cursor and kicks off the next block's generation (unless this WAS
    /// the closing block). On the 3rd consecutive fail, escalates: the
    /// failed block is marked `Escalated` (never retried again, never
    /// reveals the answer) and a differently-framed re-approach block is
    /// generated in its place.
    pub async fn submit_gate_response(&self, app: Option<&AppHandle>, block_id: &str, submission: GateSubmission) -> AppResult<GateResult> {
        let block = self.store.get_block(block_id)?.ok_or_else(|| AppError::InvalidInput(format!("block not found: {block_id}")))?;
        if !block.block_type.is_gate() {
            return Err(AppError::InvalidInput(format!("el bloque {block_id} no es una compuerta ({})", block.block_type.as_str())));
        }
        if block.block_type != submission.block_type() {
            return Err(AppError::InvalidInput("el blockType de la entrega no coincide con el bloque".to_string()));
        }
        if matches!(block.status, BlockStatus::Passed | BlockStatus::Escalated) {
            return Err(AppError::InvalidInput("esta compuerta ya fue resuelta".to_string()));
        }
        let document = self
            .store
            .get_document(&block.document_id)?
            .ok_or_else(|| AppError::InvalidInput("documento no encontrado".to_string()))?;
        if block.order_index != document.current_block_index {
            return Err(AppError::InvalidInput("este bloque ya no es el bloque activo de la clase".to_string()));
        }

        let submission_text = submission_text_of(&submission);
        let (passed, scaffold_feedback, criteria) = match &submission {
            GateSubmission::InteractivePredictionGate { selected_option } => {
                (grading::grade_prediction_gate(&block, selected_option)?, None, Vec::new())
            }
            GateSubmission::BranchingScenarioChallenge { selected_choice } => {
                (grading::grade_branching_scenario(&block, selected_choice)?, None, Vec::new())
            }
            GateSubmission::HeuristicErrorAudit { diagnosis_text } => {
                let g = grading::grade_with_model(&self.orchestrator, app, &block, diagnosis_text, block.attempt_count + 1).await?;
                (g.passed, g.scaffold_feedback, g.criteria)
            }
            GateSubmission::HandsOnMission { submission_text } => {
                let g = grading::grade_with_model(&self.orchestrator, app, &block, submission_text, block.attempt_count + 1).await?;
                (g.passed, g.scaffold_feedback, g.criteria)
            }
        };

        // Roll this outcome into the learner's cross-course cognitive memory
        // — see `domain::learner_memory`. Best-effort: a memory read/write
        // failure must never block the student's actual grading result, so
        // errors here are logged, not propagated.
        let mut memory = self.store.get_learner_memory(LOCAL_LEARNER_ID).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to load learner memory, using transient defaults");
            crate::domain::learner_memory::LearnerCognitiveMemory::new(LOCAL_LEARNER_ID)
        });
        let heavy_scaffolding = memory.needs_heavy_scaffolding();
        memory.record_gate_outcome(block.block_type, passed);
        let friction_is_low = memory.friction_is_low();

        // Append the evidence row for THIS attempt — pass, fail or the
        // attempt that tips the gate into escalation.
        let will_escalate = !passed && block.attempt_count + 1 >= if friction_is_low { 1 } else { MAX_GATE_ATTEMPTS };
        let outcome = if passed {
            EvidenceOutcome::Passed
        } else if will_escalate {
            EvidenceOutcome::Escalated
        } else {
            EvidenceOutcome::Failed
        };
        self.record_gate_evidence(&document.class_id, &block, outcome, criteria, heavy_scaffolding);
        if let Ok(Some(gen_ctx)) = self.store.class_generation_context(&document.class_id) {
            if passed {
                memory.resolve_misconceptions_for_block(block_id);
            } else if let Some(pattern) = misconception_pattern_from_submission(&block, &submission) {
                memory.record_misconception(&gen_ctx.class.title, &pattern, &iso_date_from_ms(now_ms()), block_id);
            }
        }
        if let Err(e) = self.store.save_learner_memory(&memory) {
            tracing::warn!(error = %e, "failed to persist learner memory update");
        }

        if passed {
            let updated = self.store.update_block_status(block_id, BlockStatus::Passed, None, true)?;
            if document.initial_prediction.is_none() {
                self.store.set_document_initial_prediction(&document.id, &submission_text)?;
            }
            // Absolute target (gate order + 1), not the stale read's `+1`
            // — grading can be a model call, and nothing may move the
            // cursor backwards while it runs.
            self.store.advance_cursor_to_at_least(&document.id, block.order_index + 1)?;

            // Unlike `should_auto_chain` (which judges a freshly GENERATED
            // block, deciding whether to keep buffering), `updated` here is
            // the gate that was JUST PASSED — by definition always a gate
            // type (rejected above otherwise), never the closing block. The
            // whole reason generation was paused was this exact gate; now
            // that it's resolved, the chain always resumes.
            self.spawn_background_next_block(app, document.class_id.clone(), document.id.clone());
            return Ok(GateResult {
                passed: true,
                block: grading::redact_block(&updated),
                feedback: None,
                escalation_block: None,
                next_block: None,
            });
        }

        // `cognitiveFrictionTolerance == low_frustration` (see
        // `domain::learner_memory`): escalate to a re-angled block after
        // just 1 failure instead of insisting on 3 — "ofrece
        // desescalamiento visual inmediato tras un primer fallo".
        let effective_max_attempts = if friction_is_low { 1 } else { MAX_GATE_ATTEMPTS };
        let attempt_count = block.attempt_count + 1;
        if attempt_count < effective_max_attempts {
            let updated = self.store.update_block_status(block_id, BlockStatus::Failed, scaffold_feedback.as_deref(), true)?;
            return Ok(GateResult {
                passed: false,
                block: grading::redact_block(&updated),
                feedback: scaffold_feedback,
                escalation_block: None,
                next_block: None,
            });
        }

        // 3rd fail: escalate. The failed gate is locked forever (never
        // revealed, never retried); a fresh re-approach block carries the
        // student forward instead.
        let escalated = self.store.update_block_status(block_id, BlockStatus::Escalated, scaffold_feedback.as_deref(), true)?;
        let escalation_block = generate_and_persist_block(
            &self.store,
            &self.orchestrator,
            app,
            &document.class_id,
            &document.id,
            Some(EscalationInput { failed_block: escalated.clone(), recent_wrong_submission: submission_text }),
        )
        .await?
        .ok_or_else(|| {
            AppError::Persistence("otra generación concurrente reemplazó el bloque de re-aproximación".to_string())
        })?;
        self.store
            .advance_cursor_to_at_least(&document.id, cursor_target_after_insert(escalation_block.order_index, escalation_block.block_type))?;
        if should_auto_chain(&escalation_block) {
            self.spawn_background_next_block(app, document.class_id.clone(), document.id.clone());
        }

        Ok(GateResult {
            passed: false,
            block: grading::redact_block(&escalated),
            feedback: scaffold_feedback,
            escalation_block: Some(grading::redact_block(&escalation_block)),
            next_block: None,
        })
    }

    /// Best-effort, like the memory update: failing to log evidence must
    /// never block the student's grading result.
    fn append_evidence(&self, evidence: &SkillEvidence) {
        if let Err(e) = self.store.append_skill_evidence(evidence) {
            tracing::warn!(error = %e, skill_id = %evidence.skill_id, "failed to append skill evidence");
        }
    }

    /// One row per graded gate attempt: the skill (= class), result, how many
    /// feedback rounds preceded it, the support level the gate was generated
    /// under (re-derived from the blocks that came before it) and the
    /// grader's rubric judgment.
    fn record_gate_evidence(
        &self,
        class_id: &str,
        block: &NotebookBlock,
        outcome: EvidenceOutcome,
        rubric: Vec<RubricCriterion>,
        heavy_scaffolding: bool,
    ) {
        let Some(kind) = EvidenceKind::for_block(block.block_type) else { return };
        let Ok(Some(gen_ctx)) = self.store.class_generation_context(class_id) else { return };
        let prior: Vec<NotebookBlock> = self
            .store
            .load_notebook_by_class(class_id)
            .ok()
            .flatten()
            .map(|p| p.blocks.into_iter().filter(|b| b.order_index < block.order_index).collect())
            .unwrap_or_default();
        let support = scaffolding::plan(&prior, heavy_scaffolding).support_level;
        let evidence = SkillEvidence::new(class_id, &gen_ctx.course.id, kind, outcome)
            .with_block(&block.id)
            .with_attempt(block.attempt_count + 1, block.attempt_count)
            .with_support_level(Some(support.as_str()))
            .with_transfer(block.content_json.get("isTransfer").and_then(|v| v.as_bool()).unwrap_or(false))
            .with_rubric(rubric);
        self.append_evidence(&evidence);
    }

    /// Grades the closing block's free-text reflection via
    /// `closure_feedback_grader`. The submitted reflection is persisted
    /// BEFORE grading (the frontend's debounced save may still be in
    /// flight), so the verdict and the `studentReflection` that
    /// `class_is_complete` reads are always the same text. `Passed` on the
    /// closure is what completes the class; a fail leaves it `Failed` and
    /// re-submittable. An already-approved closure never re-grades (a later
    /// fail would re-lock a class the student already earned). Never
    /// touches the cursor or the chain — the closure is the notebook's last
    /// block by definition.
    pub async fn submit_closure_feedback(
        &self,
        app: Option<&AppHandle>,
        block_id: &str,
        reflection: &str,
    ) -> AppResult<ClosureFeedback> {
        let mut block = self
            .store
            .get_block(block_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("block not found: {block_id}")))?;
        if block.block_type != DynamicBlockType::MetacognitiveClosure {
            return Err(AppError::InvalidInput(format!(
                "el bloque {block_id} no es un cierre ({})",
                block.block_type.as_str()
            )));
        }
        if matches!(block.status, BlockStatus::Passed) {
            let feedback = block.last_feedback.clone().unwrap_or_default();
            return Ok(ClosureFeedback { passed: true, feedback, block: grading::redact_block(&block) });
        }
        let reflection = reflection.trim();
        if reflection.is_empty() {
            return Err(AppError::InvalidInput("escribe tu reflexión antes de pedir el feedback".to_string()));
        }

        // Persist the exact text being graded so completion and verdict
        // can never disagree (e.g. the debounced `onSave` hadn't landed).
        let document = self
            .store
            .get_document(&block.document_id)?
            .ok_or_else(|| AppError::InvalidInput("documento no encontrado".to_string()))?;
        let mut content = block.content_json.clone();
        content["studentReflection"] = serde_json::Value::String(reflection.to_string());
        self.store
            .update_block_contents(&document.id, &[BlockUpdate { id: block.id.clone(), content_json: content.clone() }])?;
        block.content_json = content;

        let graded = grading::grade_closure_with_model(&self.orchestrator, app, &block, reflection, block.attempt_count + 1).await?;
        let status = if graded.passed { BlockStatus::Passed } else { BlockStatus::Failed };
        let updated = self.store.update_block_status(block_id, status, Some(&graded.feedback), true)?;
        if let Ok(Some(gen_ctx)) = self.store.class_generation_context(&document.class_id) {
            let outcome = if graded.passed { EvidenceOutcome::Passed } else { EvidenceOutcome::Failed };
            let evidence = SkillEvidence::new(&document.class_id, &gen_ctx.course.id, EvidenceKind::Closure, outcome)
                .with_block(block_id)
                .with_attempt(block.attempt_count + 1, block.attempt_count);
            self.append_evidence(&evidence);
        }

        // A passed closure means the class's concept is genuinely learned —
        // seed it into the spaced-retrieval queue so a FUTURE class's first
        // block can reactivate it (see `domain::learner_memory` and
        // `grounding::single_block_violations`'s due-retrieval rule).
        // Best-effort, same as the gate-outcome recording above.
        if graded.passed {
            if let Ok(Some(gen_ctx)) = self.store.class_generation_context(&document.class_id) {
                match self.store.get_learner_memory(LOCAL_LEARNER_ID) {
                    Ok(mut memory) => {
                        // First retrieval is due only after the minimum retention interval,
                        // so a prompt answered right after the class can't pose as retention.
                        memory.queue_retrieval(&document.class_id, &gen_ctx.class.title, now_ms() + MIN_RETENTION_INTERVAL_MS);
                        if let Err(e) = self.store.save_learner_memory(&memory) {
                            tracing::warn!(error = %e, "failed to persist learner memory after closure");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to load learner memory after closure"),
                }
            }
        }

        Ok(ClosureFeedback { passed: graded.passed, feedback: graded.feedback, block: grading::redact_block(&updated) })
    }

    /// Atomic retry of ONLY the pending (still-Draft-looking, i.e. the
    /// document has fewer blocks than its own `current_block_index` implies
    /// it should) block — never touches earlier persisted blocks. Used by
    /// the frontend when `notebook_block://error` fires.
    pub async fn retry_pending_block(&self, app: Option<&AppHandle>, document_id: &str) -> AppResult<()> {
        let document = self.store.get_document(document_id)?.ok_or_else(|| AppError::InvalidInput("documento no encontrado".to_string()))?;
        self.spawn_background_next_block(app, document.class_id.clone(), document.id.clone());
        Ok(())
    }

    /// Regenerates ONE already-persisted block IN PLACE — the repair path
    /// for a block whose stored JSON the renderer can't display anymore
    /// ("Este bloque no se pudo mostrar"). Everything else about the
    /// notebook stays put: same `id`/`order_index`, so the gate cursor, the
    /// pacing and the blocks behind it are untouched; same `block_type`, so
    /// the sequence (gates, closure) is unchanged; and a block the student
    /// had already passed/escalated keeps that grade. The whole class is
    /// returned so the caller can re-render every block from fresh state.
    pub async fn regenerate_block(&self, app: Option<&AppHandle>, block_id: &str) -> AppResult<NotebookPayload> {
        let broken = self
            .store
            .get_block(block_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("block not found: {block_id}")))?;
        let document = self
            .store
            .get_document(&broken.document_id)?
            .ok_or_else(|| AppError::InvalidInput("documento no encontrado".to_string()))?;
        let class_id = document.class_id.clone();
        let ctx = self
            .store
            .class_generation_context(&class_id)?
            .ok_or_else(|| AppError::InvalidInput(format!("class not found: {class_id}")))?;

        // The broken block is EXCLUDED from the context the model sees: the
        // replacement must be judged as "the block that belongs at this
        // position", against the same neighbours the original one faced.
        let mut existing_blocks = self.store.load_notebook_by_class(&class_id)?.map(|p| p.blocks).unwrap_or_default();
        existing_blocks.retain(|b| b.id != broken.id);

        let diagnostic_profile = self.store.get_course_diagnostic_battery(&ctx.course.id)?.map(|s| s.profile_summary());
        let block_type_usage = self.store.block_type_usage_for_course(&ctx.course.id, &class_id)?;
        let learner_memory = self.store.get_learner_memory(LOCAL_LEARNER_ID)?;
        let mastery = mastery_progress(&existing_blocks);
        let required = broken.block_type;
        let is_closing_block = required == DynamicBlockType::MetacognitiveClosure;

        let initial_prediction = document.initial_prediction.clone();
        let final_result_so_far = render::synthesize_final_result_so_far(&existing_blocks);
        let mut base_input = if is_closing_block {
            // The original closing block may have been generated by the
            // forced-closure path (mastery not reached yet) — reuse that
            // framing so the repair doesn't get rejected for closing early.
            render::render_forced_closure_input(&ctx, &existing_blocks, initial_prediction.as_deref(), &final_result_so_far, mastery)
        } else {
            render::render_next_block_input(
                &ctx,
                &existing_blocks,
                diagnostic_profile.as_ref(),
                &block_type_usage,
                initial_prediction.as_deref(),
                &final_result_so_far,
                mastery,
                &learner_memory,
                now_ms(),
            )
        };
        base_input.push_str(&format!(
            "\n\nNOTA_DEL_SISTEMA: este bloque REEMPLAZA a otro que no se pudo mostrar por un error de formato, en \
             la MISMA posición de la clase. Debe ser exactamente de tipo {} y la misma duración/peso; no cambies \
             ningún otro bloque.",
            required.as_str()
        ));

        let job = BlockGenerationJob {
            orchestrator: &self.orchestrator,
            app,
            class_id: class_id.clone(),
            base_input,
            ctx,
            existing_blocks,
            block_type_usage,
            mastery,
            is_escalation: false,
            force_close: is_closing_block,
            // A regeneration repairs a block that already occupies a fixed
            // position/type — the due-retrieval rule must never veto it,
            // even if a due item exists NOW (memory state can move between
            // the original generation and a later repair).
            has_due_retrieval: false,
            required_block_type: Some(required),
            is_regeneration: true,
            last_grounded: Mutex::new(None),
        };
        let report = run_generator_critic_loop(&job, MAX_ATTEMPTS, RETRY_BACKOFF_BASE).await?;
        let content_json = serde_json::to_value(&report.value).map_err(|e| AppError::Persistence(e.to_string()))?;
        let replaced = self.store.replace_block_payload(&broken.id, report.value.block_type(), &content_json)?;
        super::events::emit_block_ready(app, &broken.document_id, &grading::redact_block(&replaced));

        let refreshed = self
            .load_reconciled(&class_id)?
            .ok_or_else(|| AppError::Persistence(format!("notebook concurrently removed for {class_id}")))?;
        Ok(NotebookPayload { document: refreshed.document, blocks: grading::redact_all(refreshed.blocks) })
    }

    /// Spawns a detached background task that keeps generating+persisting
    /// blocks — one `generate_and_persist_block` call per loop iteration,
    /// not a fresh spawn per block — until it produces either a GATE (stop:
    /// wait for the student's submission) or the closing
    /// `metacognitive_closure` (stop: notebook complete). Runs with owned
    /// clones of `store`/`orchestrator`/`app` since it outlives this method
    /// call and must not borrow `&self`.
    fn spawn_background_next_block(&self, app: Option<&AppHandle>, class_id: String, document_id: String) {
        let store = self.store.clone();
        let orchestrator = Arc::clone(&self.orchestrator);
        let app_owned = app.cloned();
        tokio::spawn(run_background_chain(store, orchestrator, app_owned, class_id, document_id));
    }
}

/// Whether the notebook should keep buffering blocks past this one, WITHOUT
/// waiting for a student action. `false` for a gate (must wait for
/// `submit_gate_response`) and for `metacognitive_closure` (the notebook is
/// done — nothing generates past the closing block).
fn should_auto_chain(block: &NotebookBlock) -> bool {
    !block.block_type.is_gate() && block.block_type != DynamicBlockType::MetacognitiveClosure
}

/// The background generation loop shared by `start_class_notebook`'s
/// buffering call, every gate pass, and an escalation's follow-on chaining.
/// A free function (not a method) so it can be handed straight to
/// `tokio::spawn` with owned state.
async fn run_background_chain(
    store: NotebookStore,
    orchestrator: Arc<Orchestrator>,
    app: Option<AppHandle>,
    class_id: String,
    document_id: String,
) {
    loop {
        super::events::emit_block_generating(app.as_ref(), &document_id);
        match generate_and_persist_block(&store, &orchestrator, app.as_ref(), &class_id, &document_id, None).await {
            Ok(None) => {
                // Another writer produced this block first (the count-guard
                // discarded our candidate) — it owns the chain now; stop.
                return;
            }
            Ok(Some(block)) => {
                // Absolute, monotonic cursor advance — a gate points AT
                // itself (locked, awaiting the submission), a content block
                // unlocks itself. `MAX` keeps a stale writer from ever
                // regressing the cursor like the old relative `+1` did.
                let target = cursor_target_after_insert(block.order_index, block.block_type);
                if let Err(e) = store.advance_cursor_to_at_least(&document_id, target) {
                    tracing::warn!(document_id = %document_id, error = %e, "notebook cursor advance failed");
                    super::events::emit_block_error(app.as_ref(), &document_id, &e.to_string());
                    return;
                }
                super::events::emit_block_ready(app.as_ref(), &document_id, &grading::redact_block(&block));
                if !should_auto_chain(&block) {
                    return;
                }
                // Non-gate, non-closing: loop again to buffer the next one.
            }
            Err(e) => {
                tracing::warn!(document_id = %document_id, error = %e, "background notebook block generation failed");
                super::events::emit_block_error(app.as_ref(), &document_id, &e.to_string());
                return;
            }
        }
    }
}

/// Absolute cursor target right after inserting a block at `order_index`:
/// a GATE points the cursor AT itself (the active, still-unresolved
/// block), a content block unlocks itself (cursor one past). Absolute on
/// purpose — the old `blocks_len_after_insert`/relative-`+1` formulas were
/// computed from a read that could be stale by the time they were applied;
/// this plus `advance_cursor_to_at_least` (SQL `MAX`) cannot lose a race.
fn cursor_target_after_insert(order_index: u32, block_type: DynamicBlockType) -> u32 {
    if block_type.is_gate() { order_index } else { order_index + 1 }
}

/// Rebuilds the cursor from persisted blocks — the self-heal for a
/// document whose cursor fell behind its own blocks (a lost concurrent
/// advance). Walks blocks in order: content unlocks itself, a resolved
/// gate is passed through, an unresolved gate sitting AT the cursor stops
/// the walk (it is the active block). A gap stops the walk; a legacy gate
/// already behind the cursor is skipped without regressing. Always ≥ the
/// stored cursor for any block sequence the app itself produced.
fn reconciled_cursor(blocks: &[NotebookBlock]) -> u32 {
    let mut sorted: Vec<&NotebookBlock> = blocks.iter().collect();
    sorted.sort_by_key(|b| b.order_index);
    let mut cursor = 0u32;
    for b in sorted {
        if b.order_index > cursor {
            break;
        }
        if b.block_type.is_gate() && !matches!(b.status, BlockStatus::Passed | BlockStatus::Escalated) {
            if b.order_index == cursor {
                break;
            }
            continue;
        }
        cursor = cursor.max(b.order_index + 1);
    }
    cursor
}

struct EscalationInput {
    failed_block: NotebookBlock,
    recent_wrong_submission: String,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn submission_text_of(submission: &GateSubmission) -> String {
    match submission {
        GateSubmission::InteractivePredictionGate { selected_option } => selected_option.clone(),
        GateSubmission::BranchingScenarioChallenge { selected_choice } => selected_choice.clone(),
        GateSubmission::HeuristicErrorAudit { diagnosis_text } => diagnosis_text.clone(),
        GateSubmission::HandsOnMission { submission_text } => submission_text.clone(),
    }
}

/// Pulls a concrete, documented misconception label out of a FAILED
/// conceptual-gate submission — the wrong option's own feedback text
/// (`conceptualFeedbackMap`/branch `consequence`), never invented text. Only
/// the two deterministically-graded conceptual gates have this readily
/// available; `heuristic_error_audit`/`hands_on_mission` are graded by
/// `notebook_gate_grader`, whose free-text `rationale` never reaches this
/// layer (see `grading::grade_with_model`'s return shape) — `None` for those,
/// which simply means no `RecurringMisconception` is recorded for that
/// attempt, not an error.
fn misconception_pattern_from_submission(block: &NotebookBlock, submission: &GateSubmission) -> Option<String> {
    match submission {
        GateSubmission::InteractivePredictionGate { selected_option } => {
            block.content_json.get("conceptualFeedbackMap")?.get(selected_option)?.as_str().map(str::to_string)
        }
        GateSubmission::BranchingScenarioChallenge { selected_choice } => block
            .content_json
            .get("branches")?
            .as_array()?
            .iter()
            .find(|b| b.get("choice").and_then(|v| v.as_str()) == Some(selected_choice.as_str()))?
            .get("consequence")?
            .as_str()
            .map(str::to_string),
        GateSubmission::HeuristicErrorAudit { .. } | GateSubmission::HandsOnMission { .. } => None,
    }
}

/// Generates and persists exactly ONE block — the shared core used by the
/// synchronous first call, the background chain, and the escalation path.
/// A free function (not a method) so it can run detached inside
/// `tokio::spawn` with owned/cloned `store`/`orchestrator` instead of
/// borrowing `&NotebookService` across the spawn boundary.
///
/// Returns `Ok(None)` when the count-guarded insert detects another writer
/// inserted first (concurrent starter or parallel chain) — the caller must
/// treat that as "lost the race": no cursor advance, no chain spawn.
async fn generate_and_persist_block(
    store: &NotebookStore,
    orchestrator: &Orchestrator,
    app: Option<&AppHandle>,
    class_id: &str,
    document_id: &str,
    escalation: Option<EscalationInput>,
) -> AppResult<Option<NotebookBlock>> {
    let ctx = store
        .class_generation_context(class_id)?
        .ok_or_else(|| AppError::InvalidInput(format!("class not found: {class_id}")))?;
    let existing_blocks = store.load_notebook_by_class(class_id)?.map(|p| p.blocks).unwrap_or_default();
    let diagnostic_profile = store.get_course_diagnostic_battery(&ctx.course.id)?.map(|s| s.profile_summary());
    let block_type_usage = store.block_type_usage_for_course(&ctx.course.id, class_id)?;
    let learner_memory = store.get_learner_memory(LOCAL_LEARNER_ID)?;
    let now = now_ms();
    // The exact concept ids shown to the model as `learnerMemory.dueRetrieval`
    // — captured now so a successful `spaced_interleaved_retrieval` block can
    // reactivate exactly these (never re-derived after the fact, since the
    // due set could shift between now and persistence).
    let due_ids: Vec<String> = learner_memory.due_retrieval_items(now).iter().map(|i| i.concept_id.clone()).collect();
    // Only meaningful for the class's very FIRST block — see
    // `grounding::single_block_violations`'s hard rule.
    let has_due_retrieval = existing_blocks.is_empty() && !due_ids.is_empty();

    let is_escalation = escalation.is_some();
    // Safety valve only — see `grounding::MAX_TOTAL_BLOCKS`. Never true for
    // an escalation call (abandoning a student mid-re-approach would be
    // worse than a small overshoot past the ceiling).
    let force_close = !is_escalation && existing_blocks.len() >= grounding::MAX_TOTAL_BLOCKS;
    let mastery = mastery_progress(&existing_blocks);

    let base_input = match &escalation {
        Some(esc) => render::render_escalation_block_input(&ctx, &esc.failed_block, &esc.recent_wrong_submission),
        None => {
            let document = store.get_document(document_id)?;
            let initial_prediction = document.as_ref().and_then(|d| d.initial_prediction.clone());
            let final_result_so_far = render::synthesize_final_result_so_far(&existing_blocks);
            if force_close {
                render::render_forced_closure_input(&ctx, &existing_blocks, initial_prediction.as_deref(), &final_result_so_far, mastery)
            } else {
                render::render_next_block_input(
                    &ctx,
                    &existing_blocks,
                    diagnostic_profile.as_ref(),
                    &block_type_usage,
                    initial_prediction.as_deref(),
                    &final_result_so_far,
                    mastery,
                    &learner_memory,
                    now,
                )
            }
        }
    };

    // Snapshot BEFORE `existing_blocks` moves into the job — the insert
    // below is only allowed to append at exactly this position; if the
    // count moved while the model was generating, another writer won and
    // this candidate is discarded (`Ok(None)`).
    let expected_block_count = existing_blocks.len() as u32;

    let job = BlockGenerationJob {
        orchestrator,
        app,
        class_id: class_id.to_string(),
        base_input,
        ctx,
        existing_blocks,
        block_type_usage,
        mastery,
        is_escalation,
        force_close,
        // An escalation call never needs the due-retrieval rule: it's a
        // re-approach for an already-in-progress class, never the class's
        // first block.
        has_due_retrieval: has_due_retrieval && !is_escalation,
        required_block_type: None,
        is_regeneration: false,
        last_grounded: Mutex::new(None),
    };
    let report = run_generator_critic_loop(&job, MAX_ATTEMPTS, RETRY_BACKOFF_BASE).await?;
    let block_type = report.value.block_type();
    let content_json = serde_json::to_value(&report.value).map_err(|e| AppError::Persistence(e.to_string()))?;
    let inserted = store.insert_block_if_count(document_id, block_type, &content_json, BlockStatus::Ready, Some(expected_block_count))?;

    // Showing a `spaced_interleaved_retrieval` block is exposure, not
    // evidence: mastery only moves once the student reports an outcome
    // (`NotebookService::record_retrieval_result`).

    Ok(inserted)
}

/// One block-generation job wired into the shared generator-critic loop
/// (`orchestration::loop_runner`): `generate` is one
/// `publish_notebook_block` turn, `critique` runs the deterministic review
/// (grounding + `domain::pedagogy_guardrails`) first and only then — and
/// only for the semantically hard types (`needs_llm_critic`) — the LLM
/// critic, fail-opening if it's unavailable. `backstop` ships the last
/// candidate that passed every deterministic check once the semantic
/// critic rejected the whole budget, so a class can never dead-end on
/// judgment calls alone.
struct BlockGenerationJob<'a> {
    orchestrator: &'a Orchestrator,
    app: Option<&'a AppHandle>,
    class_id: String,
    base_input: String,
    ctx: ClassGenerationContext,
    existing_blocks: Vec<NotebookBlock>,
    block_type_usage: (HashMap<String, i64>, i64),
    mastery: MasteryProgress,
    is_escalation: bool,
    force_close: bool,
    /// Whether the learner has retrieval items due RIGHT NOW and this is the
    /// class's very first block — see `grounding::single_block_violations`'s
    /// hard rule. `false` for every escalation and regeneration call (see
    /// their own construction sites for why).
    has_due_retrieval: bool,
    /// Set only by [`NotebookService::regenerate_block`]: the exact type the
    /// candidate MUST be — the block being repaired already occupies a slot
    /// in the sequence, so the replacement can't be a different type (it
    /// would silently move a gate's or a closing block's position). Checked
    /// BEFORE anything else in [`GeneratorCriticLoop::critique`] so the
    /// candidate is rejected without spending an LLM critic turn on it.
    required_block_type: Option<DynamicBlockType>,
    /// Regenerating doesn't add a block, so the `grounding::MAX_TOTAL_BLOCKS
    /// ` safety ceiling can't be reached by it — exempt the same way an
    /// escalation is (a notebook that legitimately sits AT/over the ceiling
    /// must still be able to repair its own broken block).
    is_regeneration: bool,
    last_grounded: Mutex<Option<GeneratedSectionBlock>>,
}

#[async_trait::async_trait]
impl GeneratorCriticLoop for BlockGenerationJob<'_> {
    type Candidate = GeneratedSectionBlock;

    async fn generate(&self, note: &str) -> AppResult<Option<GeneratedSectionBlock>> {
        let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
        let turn_input = format!("{}{note}", self.base_input);
        self.orchestrator.run_notebook_block_agent(self.app, NOTEBOOK_AGENT_ID, &turn_input, capture.clone()).await?;
        let generated = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
        Ok(generated)
    }

    async fn critique(&self, candidate: &GeneratedSectionBlock) -> CriticVerdict {
        if let Some(required) = self.required_block_type {
            if candidate.block_type() != required {
                return CriticVerdict::Reject {
                    feedback: vec![format!(
                        "este bloque REEMPLAZA a uno dañado y debe ser exactamente de tipo {}, no {} — vuelve a \
                         llamar a publish_notebook_block con ese tipo",
                        required.as_str(),
                        candidate.block_type().as_str()
                    )],
                };
            }
        }
        let prior_types: Vec<DynamicBlockType> = self.existing_blocks.iter().map(|b| b.block_type).collect();
        let total_so_far = self.existing_blocks.len() + 1;
        let mut violations = single_block_violations(
            &prior_types,
            candidate,
            total_so_far,
            self.is_escalation || self.is_regeneration,
            self.mastery,
            self.force_close,
            self.has_due_retrieval,
        );
        if let Some(v) = dominance_violation(candidate, &self.block_type_usage) {
            violations.push(v);
        }
        violations.extend(scaffolding::transfer_violations(
            candidate,
            &self.existing_blocks,
            self.mastery.complete(),
            self.is_escalation || self.is_regeneration || self.force_close,
        ));
        violations.extend(block_guardrail_violations(candidate));
        if !violations.is_empty() {
            tracing::warn!(class_id = %self.class_id, ?violations, "generated block failed deterministic review");
            return CriticVerdict::Reject { feedback: violations };
        }
        *self.last_grounded.lock().unwrap_or_else(|e| e.into_inner()) = Some(candidate.clone());
        if !needs_llm_critic(candidate.block_type()) {
            return CriticVerdict::Accept;
        }
        let input = render::render_block_audit_input(&self.ctx, &self.existing_blocks, candidate);
        let capture: BlockAuditCapture = Arc::new(Mutex::new(None));
        match self.orchestrator.run_pedagogical_critic_agent(self.app, PEDAGOGICAL_CRITIC_AGENT_ID, &input, capture.clone()).await {
            Ok(_) => {
                let audit = capture.lock().unwrap_or_else(|e| e.into_inner()).take();
                match audit {
                    Some(audit) if audit.accepted => CriticVerdict::Accept,
                    Some(audit) => CriticVerdict::Reject { feedback: audit.feedback },
                    None => {
                        tracing::warn!(class_id = %self.class_id, "pedagogical critic returned no verdict, failing open");
                        CriticVerdict::Accept
                    }
                }
            }
            Err(e) => {
                tracing::warn!(class_id = %self.class_id, error = %e, "pedagogical critic unavailable, failing open");
                CriticVerdict::Accept
            }
        }
    }

    fn corrective_note(&self, cause: &Cause) -> String {
        match cause {
            Cause::Rejected { feedback } => format!(
                "\n\nNOTA_DEL_SISTEMA: tu bloque anterior tuvo estos problemas: {}. Corrígelo y \
                 vuelve a llamar a publish_notebook_block en este turno.",
                feedback.join("; ")
            ),
            Cause::EmptyAttempt => "\n\nNOTA_DEL_SISTEMA: no llamaste a publish_notebook_block. Debes llamarla en \
                                     este turno."
                .to_string(),
            Cause::OutputBudgetExhausted => "\n\nNOTA_DEL_SISTEMA: la respuesta anterior se quedó sin espacio de salida a mitad de camino \
                                             (demasiado texto). Genera el MISMO bloque de forma mucho más breve y concisa: si ibas a usar \
                                             declarative_visual_diagram con una escena compleja, simplifícala o cambia a un bloque de texto; \
                                             respeta estrictamente los límites de palabras del catálogo. Vuelve a llamar a \
                                             publish_notebook_block en este turno."
                .to_string(),
        }
    }

    fn backstop(&self, _rejections: &[Cause]) -> Option<GeneratedSectionBlock> {
        self.last_grounded.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    fn exhausted_error(&self) -> AppError {
        AppError::AgentExecutionFailed("el generador de notebooks no produjo un bloque válido".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(order_index: u32, block_type: DynamicBlockType, status: BlockStatus) -> NotebookBlock {
        NotebookBlock {
            id: format!("b{order_index}"),
            document_id: "doc".to_string(),
            block_type,
            content_json: serde_json::json!({}),
            order_index,
            status,
            attempt_count: 0,
            last_feedback: None,
        }
    }

    #[test]
    fn cursor_target_points_a_gate_at_itself_and_content_past_itself() {
        assert_eq!(cursor_target_after_insert(4, DynamicBlockType::InteractivePredictionGate), 4);
        assert_eq!(cursor_target_after_insert(4, DynamicBlockType::AnchoredMicroTheory), 5);
    }

    #[test]
    fn reconciled_cursor_rebuilds_a_cursor_that_fell_behind_its_blocks() {
        // The exact wedged shape observed in production: content blocks
        // whose relative `+1` lost a race, then gates buffered behind a
        // cursor that never caught up — every submit got rejected as
        // "not the active block".
        let blocks = vec![
            block(0, DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready),
            block(1, DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready),
            block(2, DynamicBlockType::DeclarativeVisualDiagram, BlockStatus::Ready),
            block(3, DynamicBlockType::DeclarativeVisualDiagram, BlockStatus::Ready),
            block(4, DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready),
            block(5, DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready),
        ];
        assert_eq!(reconciled_cursor(&blocks), 4, "everything before the first unresolved gate unlocks; the gate is active");
    }

    #[test]
    fn reconciled_cursor_walks_past_resolved_gates_and_stops_at_the_active_one() {
        let blocks = vec![
            block(0, DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready),
            block(1, DynamicBlockType::InteractivePredictionGate, BlockStatus::Passed),
            block(2, DynamicBlockType::HeuristicErrorAudit, BlockStatus::Escalated),
            block(3, DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready),
            block(4, DynamicBlockType::HandsOnMission, BlockStatus::Ready),
        ];
        assert_eq!(reconciled_cursor(&blocks), 3, "passed and escalated gates pass through; the walk stops AT the ready one");
        let resolved_only = &blocks[..3];
        assert_eq!(reconciled_cursor(resolved_only), 3, "all-resolved prefix walks off the end");
    }

    #[test]
    fn reconciled_cursor_never_regresses_and_stops_at_gaps_or_nothing() {
        let gap = vec![
            block(0, DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready),
            block(2, DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready),
        ];
        assert_eq!(reconciled_cursor(&gap), 1, "a gap stops the walk");
        assert_eq!(reconciled_cursor(&[]), 0);
    }
}
