import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  ClassRecordSchema,
  ClosureFeedbackSchema,
  CourseSchema,
  DiagnosticBatteryStateSchema,
  GateResultSchema,
  NotebookBlockEventSchema,
  NotebookPayloadSchema,
  RoadmapSessionSchema,
  RoadmapSessionSummarySchema,
  RoadmapTurnPayloadSchema,
} from "./schemas";
import type { GateSubmission, NotebookBlockEvent } from "./schemas";
import type { BlockUpdate } from "../types";
import type {
  AgentDefinition,
  AgentEventPayload,
  AgentOutput,
  AppError,
  ModelInfo,
  ProviderWithStatus,
  ToolInfo,
} from "../types";

export function tauriError(e: unknown): AppError {
  if (typeof e === "string") return { code: "Unknown", message: e };
  if (e && typeof e === "object" && "message" in e) {
    const e2 = e as Record<string, unknown>;
    return {
      code: typeof e2.code === "string" ? e2.code : "Unknown",
      message: String(e2.message ?? e),
    };
  }
  return { code: "Unknown", message: String(e) };
}

// --- Providers -------------------------------------------------------------

export const listProviders = () => invoke<ProviderWithStatus[]>("list_providers");

export const saveProvider = (config: {
  id: string;
  provider: string;
  name: string;
  default_model: string | null;
  base_url: string | null;
}) => invoke<ProviderWithStatus>("save_provider", { config });

export const saveProviderKey = (provider_id: string, api_key: string) =>
  invoke<boolean>("save_provider_key", { providerId: provider_id, apiKey: api_key });

export const providerHasKey = (provider_id: string) =>
  invoke<boolean>("provider_has_key", { providerId: provider_id });

export const deleteProviderKey = (provider_id: string) =>
  invoke<void>("delete_provider_key", { providerId: provider_id });

export const testProvider = (provider_id: string) =>
  invoke<void>("test_provider", { providerId: provider_id });

export const listModels = (provider_id: string) =>
  invoke<ModelInfo[]>("list_models", { providerId: provider_id });

export const suggestedModels = (provider: string) =>
  invoke<string[]>("suggested_models", { provider });

// --- Agents ----------------------------------------------------------------

export const listAgents = () => invoke<AgentDefinition[]>("list_agents");

export const createAgent = (agent: AgentDefinition) =>
  invoke<AgentDefinition>("create_agent", { agent });

export const deleteAgent = (agent_id: string) =>
  invoke<void>("delete_agent", { agentId: agent_id });

export const runAgent = (agent_id: string, input: string) =>
  invoke<AgentOutput>("run_agent", { agentId: agent_id, input });

export const listTools = () => invoke<ToolInfo[]>("list_tools");

// --- Chat / workflows ------------------------------------------------------

export const delegateAgent = (from_agent_id: string, to_agent_id: string, task: string) =>
  invoke<AgentOutput>("delegate_agent", {
    fromAgentId: from_agent_id,
    toAgentId: to_agent_id,
    task,
  });

export const runResearchToDraft = (topic: string) =>
  invoke<AgentOutput>("run_research_to_draft", { topic });

// --- Roadmap & Syllabus Diagnostic Agent ------------------------------------
//
// Every payload from the roadmap agent is parsed through the Zod schemas in
// `./schemas.ts` before the UI touches it — this is the "Generative UI"
// validation boundary: a malformed backend response fails loudly here
// instead of silently rendering `undefined` deep in a component.

export const startRoadmapSession = () =>
  invoke("start_roadmap_session").then((v) => RoadmapTurnPayloadSchema.parse(v));

export const sendRoadmapMessage = (session_id: string, message: string) =>
  invoke("send_roadmap_message", { sessionId: session_id, message }).then((v) =>
    RoadmapTurnPayloadSchema.parse(v),
  );

/**
 * Re-runs the turn that just failed (network blip / timeout) after the
 * backend's own retry budget ran out. Doesn't replay the student's input —
 * a failed turn already persisted it — so the log never duplicates.
 */
export const retryRoadmapTurn = (session_id: string) =>
  invoke("retry_roadmap_turn", { sessionId: session_id }).then((v) => RoadmapTurnPayloadSchema.parse(v));

export const getRoadmapSession = (session_id: string) =>
  invoke("get_roadmap_session", { sessionId: session_id }).then((v) => RoadmapSessionSchema.parse(v));

export const listRoadmapSessions = () =>
  invoke("list_roadmap_sessions").then((v) => RoadmapSessionSummarySchema.array().parse(v));

export const renameRoadmapSession = (session_id: string, title: string) =>
  invoke("rename_roadmap_session", { sessionId: session_id, title }).then((v) =>
    RoadmapSessionSummarySchema.parse(v),
  );

export const deleteRoadmapSession = (session_id: string) =>
  invoke<void>("delete_roadmap_session", { sessionId: session_id });

/**
 * Idempotent recovery for sessions sealed before the SQLite course import
 * existed (missing imported_course_id/first_class_id). Safe to call whenever
 * those ids are missing — a no-op if they're already set.
 */
export const ensureCourseImported = (session_id: string) =>
  invoke("ensure_course_imported", { sessionId: session_id }).then((v) => RoadmapSessionSchema.parse(v));

/**
 * Records one answer to the session's pending diagnostic battery. Once the
 * last question is answered, this also runs the Roadmap-stage turn and
 * returns an (often now-sealed) session in the same response.
 */
export const answerDiagnosticQuestion = (session_id: string, question_index: number, answer: string) =>
  invoke("answer_diagnostic_question", { sessionId: session_id, questionIndex: question_index, answer }).then((v) =>
    RoadmapTurnPayloadSchema.parse(v),
  );

/** Escape hatch: finishes the Diagnostic stage with whatever's answered so far. */
export const skipDiagnosticBattery = (session_id: string) =>
  invoke("skip_diagnostic_battery", { sessionId: session_id }).then((v) => RoadmapTurnPayloadSchema.parse(v));

// --- Notebook engine ---------------------------------------------------------
//
// Same "Generative UI" validation boundary as the roadmap agent: every
// payload from these commands is parsed through the Zod schemas in
// `./schemas.ts` before a component ever touches it.

export const importCourseFromRoadmap = (session_id: string) =>
  invoke("import_course_from_roadmap", { sessionId: session_id }).then((v) => ClassRecordSchema.array().parse(v));

export const listCourseClasses = (course_id: string) =>
  invoke("list_course_classes", { courseId: course_id }).then((v) => ClassRecordSchema.array().parse(v));

export const listCourses = () =>
  invoke("list_courses").then((v) => CourseSchema.array().parse(v));

/**
 * Starts (or resumes) a class's notebook: returns block 1 immediately, and
 * buffers subsequent blocks in the background — announced via
 * `notebook_block://*` events (see `onNotebookBlockEvent`). Calling this
 * again on an already-started class just returns current progress, never
 * regenerates.
 */
export const startClassNotebook = (class_id: string) =>
  invoke("start_class_notebook", { classId: class_id }).then((v) => NotebookPayloadSchema.parse(v));

/** Whatever's persisted so far for this class — the resume/reload path. Never triggers generation itself. */
export const getClassNotebookProgress = (class_id: string) =>
  invoke("get_class_notebook_progress", { classId: class_id }).then((v) => NotebookPayloadSchema.parse(v));

/**
 * The literal mastery gate: grades `submission` against `block_id`'s
 * content and advances (or locks/escalates) the notebook accordingly.
 */
export const submitGateResponse = (block_id: string, submission: GateSubmission) =>
  invoke("submit_gate_response", { blockId: block_id, submission }).then((v) => GateResultSchema.parse(v));

/**
 * Grades the student's WRITTEN answer to one retrieval prompt and reveals the
 * solution only now (the client never receives it before). The only path that
 * can count as retention evidence.
 */
export const submitRetrievalAnswer = (block_id: string, item_index: number, answer: string) =>
  invoke<{ correct: boolean; expectedAnswer: string }>("submit_retrieval_answer", {
    blockId: block_id,
    itemIndex: item_index,
    answer,
  });

/** "No lo recuerdo": reveals the solution and records a self-reported miss. */
export const revealRetrievalAnswer = (block_id: string, item_index: number) =>
  invoke<string>("reveal_retrieval_answer", { blockId: block_id, itemIndex: item_index });

// Grades the closing block's reflection: the backend persists the submitted
// text, stores the verdict on the block, and returns the always
// student-facing feedback. A `passed` verdict is what completes the class.
export const gradeClosureReflection = (block_id: string, reflection: string) =>
  invoke("grade_closure_reflection", { blockId: block_id, reflection }).then((v) => ClosureFeedbackSchema.parse(v));

/**
 * Atomic retry of the pending (still-generating-or-failed) block for a
 * document — call this when a `notebook_block://error` event fires.
 */
export const retryPendingBlock = (document_id: string) =>
  invoke<void>("retry_pending_block", { documentId: document_id });

/**
 * Regenerates ONE already-persisted block whose stored content the renderer
 * can't display ("formato inesperado"), replacing it in place: same id,
 * same position, same block type. Returns the refreshed class so the caller
 * can re-render every block from fresh state.
 */
export const regenerateNotebookBlock = (block_id: string) =>
  invoke("regenerate_notebook_block", { blockId: block_id }).then((v) => NotebookPayloadSchema.parse(v));

/**
 * Wipes an atypical/broken class's notebook entirely (its document and every
 * block) — the whole-class counterpart to `regenerateNotebookBlock`'s
 * single-block in-place repair, for cases a targeted swap can't fix. Does
 * NOT regenerate anything itself: the caller follows this with
 * `startClassNotebook` to generate a fresh block 1, the same call it makes
 * when opening a class for the first time.
 */
export const resetClassNotebook = (class_id: string) => invoke<void>("reset_class_notebook", { classId: class_id });

export const saveNotebookState = (notebook_id: string, blocks: BlockUpdate[]) =>
  invoke<void>("save_notebook_state", { notebookId: notebook_id, blocks });

/**
 * The course's calibration battery (generated once, alongside its
 * syllabus — never as a notebook block) plus whatever the student has
 * answered so far. `null` if this course predates the feature or its
 * syllabus generation didn't produce one.
 */
export const getCourseDiagnosticBattery = (course_id: string) =>
  invoke("get_course_diagnostic_battery", { courseId: course_id }).then((v) =>
    DiagnosticBatteryStateSchema.nullable().parse(v),
  );

export const saveDiagnosticBatteryAnswers = (course_id: string, answers: Record<string, string>) =>
  invoke<void>("save_diagnostic_battery_answers", { courseId: course_id, answers });

// --- Learner cognitive memory ------------------------------------------------
//
// Single-learner app: this always describes learner id "local" — see the
// Rust `domain::learner_memory` module doc for why. Read-only viewer data.

export type CalibratedBaseline = "novice_zero" | "novice_intuitive" | "intermediate" | "advanced";
export type AbstractionLevel = "visual_analogy" | "causal_mechanics" | "formal_symbolic";
export type FrictionTolerance = "low_frustration" | "resilient";

export interface CognitiveMetrics {
  predictionAccuracyRate: number;
  errorAuditDetectionRate: number;
  preferredAbstractionLevel: AbstractionLevel;
  cognitiveFrictionTolerance: FrictionTolerance;
}

export interface RecurringMisconception {
  domainConcept: string;
  identifiedErrorPattern: string;
  lastEncounteredDate: string;
  resolved: boolean;
}

export interface SpacedRetrievalItem {
  conceptId: string;
  conceptLabel: string;
  masteryLevel: number;
  nextDueAtMs: number;
}

export interface LearnerCognitiveMemory {
  learnerId: string;
  calibratedBaseline: CalibratedBaseline;
  cognitiveMetrics: CognitiveMetrics;
  recurringMisconceptions: RecurringMisconception[];
  retrievalSpacedQueue: SpacedRetrievalItem[];
}

export const getLearnerMemory = () => invoke<LearnerCognitiveMemory>("get_learner_memory");

// --- Lexical assistant (popover) --------------------------------------------
//
// One isolated question at a time: only `term`/`fragmentContext`/
// `keyConcepts`/`history`/`question` reach the model — never the current
// gate's answer key or session state. `answerBearingStrings` is used only
// for a deterministic server-side redaction guard, never sent into the
// model prompt itself.

export interface LexicalTurn {
  question: string;
  answer: string;
}

export const askLexicalAssistant = (
  term: string,
  fragmentContext: string | null,
  keyConcepts: string[],
  history: LexicalTurn[],
  question: string,
  answerBearingStrings: string[],
) =>
  invoke<string>("ask_lexical_assistant", {
    term,
    fragmentContext,
    keyConcepts,
    history,
    question,
    answerBearingStrings,
  });

// --- Events ----------------------------------------------------------------

export async function onAgentEvent(
  channel: "agent://started" | "agent://completed" | "agent://error" | "agent://tool",
  handler: (e: AgentEventPayload) => void,
): Promise<UnlistenFn> {
  return listen<AgentEventPayload>(channel, (evt) => handler(evt.payload));
}

/**
 * Per-block notebook generation progress — the buffering counterpart to
 * `onAgentEvent`'s per-agent-run events. Fires once for every block
 * `notebook_service::generation` persists in the background, so the
 * frontend can reveal it without polling.
 */
export async function onNotebookBlockEvent(
  channel: "notebook_block://generating" | "notebook_block://ready" | "notebook_block://error",
  handler: (e: NotebookBlockEvent) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(channel, (evt) => handler(NotebookBlockEventSchema.parse(evt.payload)));
}
