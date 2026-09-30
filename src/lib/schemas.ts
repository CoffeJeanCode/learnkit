import { z } from "zod";

// Runtime mirror of `src-tauri/src/domain/roadmap.rs`. This is the
// "Generative UI" validation boundary: every JSON payload the backend agent
// produces via real tool calls is parsed against these schemas before the UI
// trusts it. Keep field names and enum values in lockstep with the Rust
// `serde` output — the cards below are camelCase on the wire
// (`#[serde(rename_all = "camelCase")]` in Rust); everything else stays
// snake_case.

export const RoadmapPhaseSchema = z.enum(["onboarding", "diagnostic", "roadmap"]);
export const SessionStatusSchema = z.enum(["active", "sealed"]);

// --- Gate cards ----------------------------------------------------------
// The actual deliverable of each gate: structured cards the UI saves and
// renders in the learner's profile the moment they arrive.

// A CLOSED set the student must pick from explicitly — Gate 1 never infers
// this from vague phrasing (see `EntryLevel` in
// `src-tauri/src/domain/roadmap.rs`). `absolute_zero` is also the only value
// that skips Gate 2 (the situational battery) entirely.
export const EntryLevelSchema = z.enum(["absolute_zero", "theoretical_foundations", "applied_intermediate"]);

export const ENTRY_LEVEL_LABEL: Record<string, string> = {
  absolute_zero: "Cero absoluto",
  theoretical_foundations: "Fundamentos teóricos",
  applied_intermediate: "Nivel medio que busca aplicar",
};

export const LearnerProfileCardSchema = z.object({
  topic: z.string(),
  targetGoal: z.string(),
  timeframeWeeks: z.number(),
  weeklyCommitmentHours: z.number(),
  totalAvailableHours: z.number(),
  entryLevel: EntryLevelSchema,
});

export const DiagnosticSummaryCardSchema = z.object({
  coreFocus: z.string(),
  identifiedNeeds: z.array(z.string()),
  learningStrategy: z.string(),
});

// Closed taxonomy for what KIND of artifact a `Micromodule.deliverable`
// actually is — mirrors `DeliverableArtifactType` in
// `src-tauri/src/domain/roadmap.rs`. Replaces a denylist-only vague-phrase
// check with a structural one: 5 of the 6 variants are self-describing,
// `other` is the deliberate escape hatch (held to a stricter minimum
// `description` length below).
export const DeliverableArtifactTypeSchema = z.enum([
  "tests_passing",
  "formal_diagram",
  "functional_cli",
  "diagnostic_matrix",
  "working_demo",
  "other",
]);

const OTHER_DELIVERABLE_MIN_LEN = 20;

// A session's concrete, checkable artifact — see `Deliverable` in Rust.
// Mirrors the server-side `deliverable_violations` rule: `description` must
// be non-empty, and when `artifactType` is `other` (the one bucket that
// isn't already self-describing) it must ALSO clear a higher minimum length,
// so "otro" can't be the whole answer.
export const DeliverableSchema = z
  .object({
    artifactType: DeliverableArtifactTypeSchema,
    description: z.string().min(1),
  })
  .superRefine((deliverable, ctx) => {
    if (deliverable.artifactType === "other" && deliverable.description.trim().length < OTHER_DELIVERABLE_MIN_LEN) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["description"],
        message: `artifactType "other" requiere una description de al menos ${OTHER_DELIVERABLE_MIN_LEN} caracteres`,
      });
    }
  });

// One of the week's exactly-2 study sessions (<=4h each, homogeneous) — the
// anti-monolith unit, each ending in its OWN authentic, verifiable artifact
// (see `Micromodule` in `src-tauri/src/domain/roadmap.rs`).
export const MicromoduleSchema = z.object({
  label: z.string(),
  hours: z.number(),
  deliverable: DeliverableSchema,
  // Central concepts + cause-effect this session explores, landed in the
  // real friction it resolves. Nullish because sessions sealed before
  // `Micromodule::focus` existed deserialize without the key.
  focus: z.string().nullish(),
  // What the student will be ABLE TO DO after this session - shown in the
  // plan and in the class header. Nullish because sessions sealed before
  // `Micromodule::objective` existed deserialize without the key.
  objective: z.string().nullish(),
  // Pure backend metadata for `notebook_generator` — never rendered by the
  // frontend (see `Micromodule::interactive_blocks`).
  interactiveBlocks: z.array(z.string()),
});

export const MilestoneSchema = z.object({
  week: z.number(),
  title: z.string(),
  // Week-level rollup — the real decomposition lives in `micromodules`.
  deliverable: z.string(),
  // Backward Design's weekly goal: the real problem this week resolves and
  // the capability it unlocks. Nullish because sessions sealed before
  // `Milestone::weekly_goal` existed deserialize without the key.
  weeklyGoal: z.string().nullish(),
  micromodules: z.array(MicromoduleSchema),
});

// The terminal transfer project the whole roadmap builds toward, per
// Backward Design (Wiggins & McTighe) — distinct from the last week's
// `Milestone`, which is still just that week's rollup (see `CapstoneProject`
// in `src-tauri/src/domain/roadmap.rs`).
export const CapstoneProjectSchema = z.object({
  title: z.string(),
  description: z.string(),
  verifiableEvidence: z.string(),
});

// Exactly what the model emits at Gate 3 — no ids or timestamps.
export const RoadmapSyllabusPackageSchema = z.object({
  courseTitle: z.string(),
  totalWeeks: z.number(),
  paceHoursPerWeek: z.number(),
  milestones: z.array(MilestoneSchema),
  capstoneProject: CapstoneProjectSchema,
});

// --- Diagnostic battery ----------------------------------------------------
// A calibration battery generated ONCE, alongside the syllabus itself — NOT
// a notebook block, and never regenerated per class (see `DiagnosticBattery`
// in `src-tauri/src/domain/notebook.rs`). It belongs to the roadmap/course,
// so its results can shape both the syllabus and every class within it.

export const DiagnosticDimensionSchema = z.enum(["intuition", "mechanics", "critical_case", "boundary"]);

export const DiagnosticQuestionSchema = z.object({
  dimension: DiagnosticDimensionSchema,
  prompt: z.string(),
  options: z.array(z.string()),
  correctOption: z.string(),
  diagnosticInsight: z.string(),
});

// Mirrors the <15% option-length-variance rule enforced server-side in
// `diagnostic_battery_violations` (src-tauri/.../roadmap_service/grounding.rs).
// Deliberately NOT a `.superRefine` on `DiagnosticQuestionSchema`: that schema
// also parses ALREADY-PERSISTED batteries on every load
// (`getCourseDiagnosticBattery`, via `.parse()`), including ones generated
// before this rule existed or under a since-fixed grounding bug — a throwing
// refine there would brick loading a student's in-progress battery instead of
// just flagging newly-generated content. Call this explicitly wherever fresh
// LLM-generated options should be linted before use, not from the parse path.
export function optionLengthVarianceViolation(options: string[]): string | null {
  const lengths = options.map((option) => option.length);
  const maxLength = Math.max(...lengths, 0);
  const minLength = options.length > 0 ? Math.min(...lengths) : 0;
  if (maxLength > 0 && (maxLength - minLength) / maxLength > 0.15) {
    return `Paridad métrica violada: variación ${(((maxLength - minLength) / maxLength) * 100).toFixed(1)}% > 15%`;
  }
  return null;
}

export const DiagnosticBatterySchema = z.object({
  goalAlignment: z.string(),
  questions: z.array(DiagnosticQuestionSchema),
});

// The persisted, course-scoped shape: the battery plus whatever the student
// has answered so far (keyed by question index as a string) — what
// `get_course_diagnostic_battery`/`save_diagnostic_battery_answers` read and
// write (see `DiagnosticBatteryState` in Rust).
export const DiagnosticBatteryStateSchema = DiagnosticBatterySchema.extend({
  answers: z.record(z.string(), z.string()).default({}),
});

// The Gate 3 card plus backend-generated bookkeeping — self-sufficient
// context for the future Notebook Builder agent.
export const SealedRoadmapSchema = z.object({
  schema_version: z.string(),
  package_id: z.string(),
  session_id: z.string(),
  generated_at_ms: z.number(),
  learner_profile: LearnerProfileCardSchema,
  diagnostic_summary: DiagnosticSummaryCardSchema,
  syllabus: RoadmapSyllabusPackageSchema,
  // The as-generated snapshot — `null` for the absolute_zero path (Gate 2 is
  // skipped entirely). The LIVE copy (with answers) lives on the imported
  // course, fetched via `getCourseDiagnosticBattery`.
  diagnostic_battery: DiagnosticBatterySchema.nullable(),
});

// What Gate 3a (propose_syllabus_plan) stores while awaiting the student's
// free-text confirmation — see `ProposedPlan` in Rust. `null` once Gate 3b
// (confirm_syllabus_plan) seals the session.
export const ProposedPlanSchema = z.object({
  diagnosticSummary: DiagnosticSummaryCardSchema,
  syllabus: RoadmapSyllabusPackageSchema,
  closingQuestion: z.string(),
});

// The agent is autonomous (real tool-calling — see `roadmap_agent.rs`)
// across a Strict Gated Flow: Gate 1 (submit_diagnostic_assessment, explicit
// entryLevel) -> Gate 2 (present_diagnostic_battery, skipped for
// absolute_zero — a genuine wait for the student's answers via
// answerDiagnosticQuestion) -> Gate 3a (propose_syllabus_plan, triggered
// automatically once Gate 2 finishes — a PROPOSAL only, nothing persisted)
// -> Gate 4 (confirm_syllabus_plan, triggered by the student's free-text
// confirmation to the proposal's closing question — THIS seals the session
// and imports the course; no notebook content is generated here, every
// class gets its notebook lazily via `startClassNotebook` on first open).
// `draft`
// accumulates the raw messages the student has sent so far (see
// `record_message` in Rust) — legitimately loose/free-form, so it stays
// loosely typed here. Only the cards above, once emitted, are validated
// strictly against schemas.
// One saved conversation turn (see `ChatTurn` in Rust) — same role
// vocabulary as the UI's ChatMessage, so restoring is a straight mapping.
export const ChatTurnSchema = z.object({
  role: z.string(),
  text: z.string(),
  at_ms: z.number(),
});

export const RoadmapSessionSchema = z.object({
  session_id: z.string(),
  phase: RoadmapPhaseSchema,
  status: SessionStatusSchema,
  turn_count_in_phase: z.number(),
  draft: z.record(z.string(), z.unknown()),
  learner_profile_card: LearnerProfileCardSchema.nullable(),
  diagnostic_summary_card: DiagnosticSummaryCardSchema.nullable(),
  roadmap_package: SealedRoadmapSchema.nullable(),
  // Set the moment `present_diagnostic_battery` is called (phase:
  // "diagnostic") — the battery + in-progress answers, before a course
  // exists to own it. `null` once the Roadmap stage seals the session (the
  // live copy then lives on the imported course — see
  // `getCourseDiagnosticBattery`).
  pending_diagnostic_battery: DiagnosticBatteryStateSchema.nullable(),
  // Set the moment `propose_syllabus_plan` (Gate 3a) succeeds — the
  // diagnostic summary + syllabus awaiting the student's free-text
  // confirmation. `null` once `confirm_syllabus_plan` (Gate 3b) seals the
  // session; nothing is persisted into SQLite until then.
  proposed_plan: ProposedPlanSchema.nullable(),
  // Set once the session seals: the SQLite course this plan was imported
  // into, and its first class — no notebook content exists for it yet
  // (every class, including the first, is lazy now), the frontend triggers
  // that the first time it navigates there via `startClassNotebook`.
  imported_course_id: z.string().nullable(),
  first_class_id: z.string().nullable(),
  // Internal grounding-rejection bookkeeping (see `roadmap_service.rs`) —
  // not rendered, kept here only so this schema stays a faithful mirror of
  // the Rust struct.
  last_rejection_reasons: z.array(z.string()),
  // Full persisted conversation (user + assistant turns) — `#[serde(default)]`
  // in Rust, so `.default([])` here keeps pre-log snapshots loadable.
  messages: z.array(ChatTurnSchema).default([]),
  // Student-chosen label for the session (rename in the drawer) — feeds the
  // header's active-session chip via `displaySessionTitle`.
  custom_title: z.string().nullable(),
  created_at_ms: z.number(),
  updated_at_ms: z.number(),
});

export const RoadmapTurnPayloadSchema = z.object({
  message: z.string(),
  session: RoadmapSessionSchema,
});

// Lightweight row for the saved-sessions drawer (see
// `RoadmapSessionSummary` in Rust).
export const RoadmapSessionSummarySchema = z.object({
  session_id: z.string(),
  phase: RoadmapPhaseSchema,
  status: SessionStatusSchema,
  title: z.string(),
  created_at_ms: z.number(),
  updated_at_ms: z.number(),
});

export type RoadmapPhase = z.infer<typeof RoadmapPhaseSchema>;
export type EntryLevel = z.infer<typeof EntryLevelSchema>;
export type LearnerProfileCard = z.infer<typeof LearnerProfileCardSchema>;
export type DiagnosticSummaryCard = z.infer<typeof DiagnosticSummaryCardSchema>;
export type RoadmapSyllabusPackage = z.infer<typeof RoadmapSyllabusPackageSchema>;
export type Milestone = z.infer<typeof MilestoneSchema>;
export type DeliverableArtifactType = z.infer<typeof DeliverableArtifactTypeSchema>;
export type Deliverable = z.infer<typeof DeliverableSchema>;
export type Micromodule = z.infer<typeof MicromoduleSchema>;
export type CapstoneProject = z.infer<typeof CapstoneProjectSchema>;
export type SealedRoadmap = z.infer<typeof SealedRoadmapSchema>;
export type ProposedPlan = z.infer<typeof ProposedPlanSchema>;
export type ChatTurn = z.infer<typeof ChatTurnSchema>;
export type RoadmapSession = z.infer<typeof RoadmapSessionSchema>;
export type RoadmapTurnPayload = z.infer<typeof RoadmapTurnPayloadSchema>;
export type RoadmapSessionSummary = z.infer<typeof RoadmapSessionSummarySchema>;

// --- Notebook engine ---------------------------------------------------------
// Runtime mirror of `src-tauri/src/domain/notebook.rs` — the relational,
// SQLite-backed course/class/notebook model. `block_type` is snake_case
// (Rust `#[serde(rename_all = "snake_case")]`); everything else stays
// snake_case to match the Rust struct field names verbatim (unlike the
// roadmap gate cards, nothing here has a camelCase wire contract) — except
// the block CONTENT itself (`DynamicSectionBlockSchema` below), which is
// camelCase, matching the tool-calling contract the agent writes it through.
//
// No fixed 4-section mold: a class's notebook is a dynamically composed,
// ordered sequence of pedagogical blocks (see `notebook_agent`'s system
// prompt) — `DynamicBlockTypeSchema` is the catalog it picks from.

export const DynamicBlockTypeSchema = z.enum([
  "spaced_interleaved_retrieval",
  "anchored_micro_theory",
  "declarative_visual_diagram",
  "branching_scenario_challenge",
  "heuristic_error_audit",
  "interactive_prediction_gate",
  "hands_on_mission",
  "metacognitive_closure",
]);
// Formalization of the 5 epistemological/disciplinary profiles that
// `src-tauri/src/agents/block_generator_agent.rs`'s system prompt describes
// in prose (see "COMPOSICIÓN DINÁMICA") to pick a starting disposition for a
// class's block sequence. Mirrors `domain::lesson_composition::
// DisciplineProfile` in Rust, which is the actual source of truth (and where
// each profile's suggested `DynamicBlockType` sequence + unit tests live).
// Documentation/typing only on this side — nothing currently reads or writes
// this value at runtime.
export const DisciplineProfileSchema = z.enum([
  "spatial_biological",
  "quantitative_math",
  "cyclic_systemic",
  "software_debugging",
  "leadership_decisions",
]);
export const NotebookStatusSchema = z.enum(["draft", "ready"]);
// Per-BLOCK gating state — see `BlockStatus` in Rust. `ready` means
// different things per block family: for a content block (theory/diagram)
// it's already unlocked; for a gate it's shown but awaiting its first
// submission, still locked.
export const BlockStatusSchema = z.enum(["ready", "passed", "failed", "escalated"]);

// --- Static, declarative visuals (arXiv:2605.30174) -----------------------
// Explicitly NOT dynamic simulators — 3 stable rendering engines, chosen per
// the concept's shape. See `blocks/StaticVisual.tsx` for how each renders.

export const MermaidChartTypeSchema = z.enum(["flowchart", "sequenceDiagram", "classDiagram", "stateDiagram", "erDiagram"]);
export const SvgTagSchema = z.enum(["rect", "circle", "line", "path", "text"]);

export const SvgElementSchema = z.object({
  tag: SvgTagSchema,
  props: z.record(z.string(), z.unknown()),
  // Nullish, not optional: Rust's `SvgElement::label` is `Option<String>` and
  // the model emits an explicit `"label": null` for every non-text element
  // (rect, line, path). `.optional()` only tolerates the ABSENT key, so a
  // null label used to fail the whole block parse and render it as
  // "formato inesperado".
  label: z.string().nullish(),
});

// A semantic cluster of elements in a scientific illustration (e.g. one
// anatomical layer or physical zone) — see `SvgGroup` in
// `src-tauri/src/domain/notebook.rs`.
export const SvgGroupSchema = z.object({
  groupId: z.string(),
  pedagogicalRole: z.string(),
  elements: z.array(SvgElementSchema),
});

export const StaticVisualSpecSchema = z.discriminatedUnion("renderEngine", [
  z.object({
    renderEngine: z.literal("mermaid"),
    chartType: MermaidChartTypeSchema,
    code: z.string(),
    caption: z.string(),
  }),
  z.object({
    renderEngine: z.literal("declarative_svg"),
    viewBox: z.string().default("0 0 800 450"),
    elements: z.array(SvgElementSchema).default([]),
    groups: z.array(SvgGroupSchema).default([]),
    pedagogicalFocus: z.string().nullish(),
    caption: z.string(),
  }),
  z.object({
    renderEngine: z.literal("conceptual_matrix"),
    headers: z.array(z.string()),
    rows: z.array(z.array(z.string())),
    contrastFocus: z.string(),
  }),
]);

// --- Dynamic pedagogical block catalog -------------------------------------

export const WalkthroughStepSchema = z.object({
  stepNumber: z.number(),
  targetVisualElement: z.string(),
  pedagogicalInsight: z.string(),
});

export const FlawedRepresentationSchema = z.object({
  context: z.string(),
  buggySnippetOrDiagram: z.string(),
  errorType: z.enum(["syntax", "conceptual_misunderstanding", "structural_inversion"]),
});

// Client-visible shape only — the backend strips `isOptimal` before this
// ever reaches the frontend (see `notebook_service::grading::redact_block`
// in Rust): it's the answer key for `branching_scenario_challenge`'s gate,
// graded server-side via `submitGate`, never sent to the client at all
// (fixes a pre-existing leak where the whole-notebook era shipped it raw).
export const ScenarioBranchSchema = z.object({
  choice: z.string(),
  consequence: z.string(),
});

// A prior-class concept reactivated in a `spaced_interleaved_retrieval`
// block — see `RetrievalPrompt` in `src-tauri/src/domain/notebook.rs`.
// `expectedAnswer` is NOT an answer key needing redaction: this block is
// self-check, not a graded gate (see `DynamicBlockType::
// SpacedInterleavedRetrieval`'s doc comment) — the frontend hides it behind
// a reveal interaction purely for the testing-effect UX, not for security.
export const RetrievalPromptSchema = z.object({
  conceptLabel: z.string(),
  prompt: z.string(),
  expectedAnswer: z.string(),
});

export const PredictionComparisonSchema = z.object({
  initialPrediction: z.string(),
  finalResult: z.string(),
  contrastNarrative: z.string(),
});

export const DynamicSectionBlockSchema = z.discriminatedUnion("blockType", [
  z.object({
    blockType: z.literal("spaced_interleaved_retrieval"),
    items: z.array(RetrievalPromptSchema),
  }),
  z.object({
    blockType: z.literal("anchored_micro_theory"),
    title: z.string(),
    intuitiveHook: z.string(),
    // Optional — only present when the model explicitly names what the
    // Capa-1 analogy does NOT cover technically. Nullish (not `.optional()`
    // alone) — Rust's `Option<String>` takes an explicit `null`, matching
    // the same convention as `visualAid` above — so old persisted
    // `anchored_micro_theory` blocks generated before this field existed
    // (which lack the key entirely) still parse, and don't break rendering.
    analogyBoundary: z.string().nullish(),
    systemRule: z.string(),
    frequentError: z.string(),
  }),
  z.object({
    blockType: z.literal("declarative_visual_diagram"),
    title: z.string(),
    visualAid: StaticVisualSpecSchema,
    guidedWalkthrough: z.array(WalkthroughStepSchema),
  }),
  z.object({
    blockType: z.literal("branching_scenario_challenge"),
    scenario: z.string(),
    decisionPoint: z.string(),
    branches: z.array(ScenarioBranchSchema),
    // Optional — only present when a diagram genuinely clarifies the
    // scenario itself (see `notebook_agent`'s system prompt).
    // Nullish — Rust's `Option<StaticVisualSpec>` takes an explicit null.
    visualAid: StaticVisualSpecSchema.nullish(),
  }),
  z.object({
    blockType: z.literal("heuristic_error_audit"),
    instruction: z.string(),
    flawedRepresentation: FlawedRepresentationSchema,
    guidingQuestions: z.array(z.string()),
    // Redacted (absent) until the gate is actually passed — see
    // `redact_block` — so this can't be required.
    modelSolution: z.string().optional(),
    // Optional — for a flaw that's spatial/visual rather than purely textual.
    // Nullish — Rust's `Option<StaticVisualSpec>` takes an explicit null.
    visualAid: StaticVisualSpecSchema.nullish(),
  }),
  z.object({
    blockType: z.literal("interactive_prediction_gate"),
    question: z.string(),
    options: z.array(z.string()),
    conceptualFeedbackMap: z.record(z.string(), z.string()),
    // Deliberately NO `correctOption` field — the answer key never reaches
    // the client at all (see `redact_block`), enforced here at the type
    // level too.
    // Optional — only when the prediction genuinely depends on a diagram.
    // Nullish — Rust's `Option<StaticVisualSpec>` takes an explicit null.
    visualAid: StaticVisualSpecSchema.nullish(),
  }),
  z.object({
    blockType: z.literal("hands_on_mission"),
    challengeStatement: z.string(),
    expectedMilestoneArtifact: z.string(),
    constraints: z.array(z.string()),
    scaffoldingHints: z.array(z.string()),
    evaluationRubricSummary: z.array(z.string()),
    // Optional — a reference diagram of the target artifact, when it helps.
    // Nullish — Rust's `Option<StaticVisualSpec>` takes an explicit null.
    visualAid: StaticVisualSpecSchema.nullish(),
  }),
  z.object({
    blockType: z.literal("metacognitive_closure"),
    synthesisTask: z.string(),
    // Optional for backward compatibility only: notebooks generated before
    // the mastery-gated redesign have no predictionComparison at all. Every
    // NEW closure always has it (the backend grounding-rejects one that
    // doesn't) — see `MetacognitiveClosureBlock.tsx` for the fallback render.
    predictionComparison: PredictionComparisonSchema.optional(),
    selfEvaluationChecklist: z.array(z.string()),
    // Local/persisted free-text reflection — non-gating, saved via
    // `save_notebook_state` same as before.
    studentReflection: z.string().optional(),
  }),
]);

export const CourseSchema = z.object({
  id: z.string(),
  title: z.string(),
  target_goal: z.string(),
  total_weeks: z.number(),
  created_at_ms: z.number(),
});

export const SyllabusMilestoneSchema = z.object({
  id: z.string(),
  course_id: z.string(),
  week_number: z.number(),
  title: z.string(),
  deliverable: z.string(),
});

export const ClassRecordSchema = z.object({
  id: z.string(),
  milestone_id: z.string(),
  class_number: z.number(),
  title: z.string(),
  order_index: z.number(),
  // This micromodule's own bounded (<=4h) allocation — one class per
  // micromodule now, not one per week (see `Micromodule` above).
  hours: z.number(),
  // The micromodule's learning objective ("qué sabrás hacer al terminar
  // esta clase") — null for classes imported before it existed, in which
  // case the notebook header shows no objective line at all.
  objective: z.string().nullish(),
  // Derived server-side (`class_is_complete`): every gate resolved plus a
  // revealed closure with a written reflection — the state that unlocks the
  // NEXT class in the sequential path. Older payloads omit it (→ false).
  complete: z.boolean().default(false),
});

export const NotebookDocumentSchema = z.object({
  id: z.string(),
  class_id: z.string(),
  title: z.string(),
  status: NotebookStatusSchema,
  updated_at_ms: z.number(),
  // The model's own justification for this notebook's block composition —
  // `null` for documents generated before this field existed.
  pedagogical_rationale: z.string().nullable(),
  // The literal gate cursor: blocks past this index don't exist client-side
  // yet — see `NotebookDocument::current_block_index` in Rust.
  current_block_index: z.number().default(0),
  // The student's answer, verbatim, to the first gate they ever answered in
  // this notebook — feeds the closing block's `predictionComparison`.
  initial_prediction: z.string().nullable().default(null),
});

// `content_json` stays loosely typed at the wire boundary (a raw record, not
// `DynamicSectionBlockSchema`): the backend genuinely stores arbitrary JSON
// there, and validating it strictly here would SILENTLY STRIP fields a
// strict schema doesn't know about (e.g. `selectedOption` merged in by an
// edit) on the very next load. The block renderer registry parses each
// block's `content_json` against `DynamicSectionBlockSchema` itself
// (`.safeParse`), falling back to an "unsupported block" card instead of
// crashing on a mismatch — see `features/notebook/blocks/index.tsx`.
export const NotebookBlockSchema = z.object({
  id: z.string(),
  document_id: z.string(),
  block_type: DynamicBlockTypeSchema,
  content_json: z.record(z.string(), z.unknown()),
  order_index: z.number(),
  status: BlockStatusSchema,
  attempt_count: z.number().default(0),
  // The most recent scaffold hint / re-approach rationale shown for this
  // block, if it's ever failed. `null`/absent for a block that's never
  // needed grading or passed on the first try.
  last_feedback: z.string().nullable().optional(),
});

export const NotebookPayloadSchema = z.object({
  document: NotebookDocumentSchema,
  blocks: z.array(NotebookBlockSchema),
});

// --- Mastery gates -----------------------------------------------------------
// What the student submits to a gate (`submitGateResponse`) and what comes
// back (`GateResult` in Rust) — the literal "backend certifies mastery"
// contract. One submission variant per gate-eligible block type; tag values
// match `DynamicBlockTypeSchema`.

export const GateSubmissionSchema = z.discriminatedUnion("blockType", [
  z.object({ blockType: z.literal("interactive_prediction_gate"), selectedOption: z.string() }),
  z.object({ blockType: z.literal("branching_scenario_challenge"), selectedChoice: z.string() }),
  z.object({ blockType: z.literal("heuristic_error_audit"), diagnosisText: z.string() }),
  z.object({ blockType: z.literal("hands_on_mission"), submissionText: z.string() }),
]);

export const GateResultSchema = z.object({
  passed: z.boolean(),
  block: NotebookBlockSchema,
  // A scaffold hint — present only on a failed attempt 1 or 2.
  feedback: z.string().nullable().optional(),
  // Populated only on the 3rd consecutive failure: a freshly generated,
  // differently-framed block re-approaching the same concept. The failed
  // block itself (`block` above) is `status: "escalated"`, never retried.
  escalation_block: NotebookBlockSchema.nullable().optional(),
  // Populated only if the background-buffered next block had already
  // finished generating by the time this submission was graded.
  next_block: NotebookBlockSchema.nullable().optional(),
});

// What comes back from `gradeClosureReflection` (`ClosureFeedback` in Rust):
// the verdict on the closing block's free-text reflection. Unlike
// `GateResult.feedback`, `feedback` is ALWAYS student-facing (pass or fail),
// and `passed: true` is what completes the class.
export const ClosureFeedbackSchema = z.object({
  passed: z.boolean(),
  feedback: z.string(),
  block: NotebookBlockSchema,
});

// Per-block progress events (`notebook_block://*`) — the buffering
// counterpart to `AgentEventSchema`'s per-agent-run events. See
// `NotebookBlockEvent` in Rust.
export const NotebookBlockEventSchema = z.object({
  event: z.enum(["generating", "ready", "error"]),
  document_id: z.string(),
  block: NotebookBlockSchema.nullable().optional(),
  message: z.string().nullable().optional(),
  retriable: z.boolean().nullable().optional(),
});

export type DynamicBlockType = z.infer<typeof DynamicBlockTypeSchema>;
export type DisciplineProfile = z.infer<typeof DisciplineProfileSchema>;
export type StaticVisualSpec = z.infer<typeof StaticVisualSpecSchema>;
export type SvgElement = z.infer<typeof SvgElementSchema>;
export type SvgGroup = z.infer<typeof SvgGroupSchema>;
export type DiagnosticDimension = z.infer<typeof DiagnosticDimensionSchema>;
export type DiagnosticQuestion = z.infer<typeof DiagnosticQuestionSchema>;
export type DiagnosticBattery = z.infer<typeof DiagnosticBatterySchema>;
export type DiagnosticBatteryState = z.infer<typeof DiagnosticBatteryStateSchema>;
export type ScenarioBranch = z.infer<typeof ScenarioBranchSchema>;
export type RetrievalPrompt = z.infer<typeof RetrievalPromptSchema>;
export type DynamicSectionBlock = z.infer<typeof DynamicSectionBlockSchema>;
export type Course = z.infer<typeof CourseSchema>;
export type SyllabusMilestone = z.infer<typeof SyllabusMilestoneSchema>;
export type ClassRecord = z.infer<typeof ClassRecordSchema>;
export type NotebookDocument = z.infer<typeof NotebookDocumentSchema>;
export type NotebookBlock = z.infer<typeof NotebookBlockSchema>;
export type NotebookPayload = z.infer<typeof NotebookPayloadSchema>;
export type BlockStatus = z.infer<typeof BlockStatusSchema>;
export type GateSubmission = z.infer<typeof GateSubmissionSchema>;
export type GateResult = z.infer<typeof GateResultSchema>;
export type ClosureFeedback = z.infer<typeof ClosureFeedbackSchema>;
export type NotebookBlockEvent = z.infer<typeof NotebookBlockEventSchema>;
export type PredictionComparison = z.infer<typeof PredictionComparisonSchema>;
