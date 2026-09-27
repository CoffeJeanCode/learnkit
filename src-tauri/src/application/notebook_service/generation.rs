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
use crate::domain::notebook::{
    BlockStatus, BlockUpdate, ClassRecord, ClosureFeedback, DynamicBlockType, GateResult, GateSubmission, GeneratedSectionBlock,
    NotebookBlock, NotebookPayload,
};
use crate::domain::pedagogy_guardrails::{block_guardrail_violations, needs_llm_critic};
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
        let (passed, scaffold_feedback) = match &submission {
            GateSubmission::InteractivePredictionGate { selected_option } => (grading::grade_prediction_gate(&block, selected_option)?, None),
            GateSubmission::BranchingScenarioChallenge { selected_choice } => (grading::grade_branching_scenario(&block, selected_choice)?, None),
            GateSubmission::HeuristicErrorAudit { diagnosis_text } => {
                grading::grade_with_model(&self.orchestrator, app, &block, diagnosis_text, block.attempt_count + 1).await?
            }
            GateSubmission::HandsOnMission { submission_text } => {
                grading::grade_with_model(&self.orchestrator, app, &block, submission_text, block.attempt_count + 1).await?
            }
        };

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

        let attempt_count = block.attempt_count + 1;
        if attempt_count < MAX_GATE_ATTEMPTS {
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

fn submission_text_of(submission: &GateSubmission) -> String {
    match submission {
        GateSubmission::InteractivePredictionGate { selected_option } => selected_option.clone(),
        GateSubmission::BranchingScenarioChallenge { selected_choice } => selected_choice.clone(),
        GateSubmission::HeuristicErrorAudit { diagnosis_text } => diagnosis_text.clone(),
        GateSubmission::HandsOnMission { submission_text } => submission_text.clone(),
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
        last_grounded: Mutex::new(None),
    };
    let report = run_generator_critic_loop(&job, MAX_ATTEMPTS, RETRY_BACKOFF_BASE).await?;
    let content_json = serde_json::to_value(&report.value).map_err(|e| AppError::Persistence(e.to_string()))?;
    store.insert_block_if_count(
        document_id,
        report.value.block_type(),
        &content_json,
        BlockStatus::Ready,
        Some(expected_block_count),
    )
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
        let prior_types: Vec<DynamicBlockType> = self.existing_blocks.iter().map(|b| b.block_type).collect();
        let total_so_far = self.existing_blocks.len() + 1;
        let mut violations =
            single_block_violations(&prior_types, candidate, total_so_far, self.is_escalation, self.mastery, self.force_close);
        if let Some(v) = dominance_violation(candidate, &self.block_type_usage) {
            violations.push(v);
        }
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
