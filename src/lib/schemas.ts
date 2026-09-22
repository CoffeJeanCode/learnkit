import { z } from "zod";

// Runtime mirror of `src-tauri/src/domain/roadmap.rs`. This is the
// "Generative UI" validation boundary: every JSON payload the backend agent
// produces (directly, or via the `roadmap_state` block it must emit) is
// parsed against these schemas before the UI trusts it. Keep field names and
// enum values in lockstep with the Rust `serde` output (snake_case, no
// renames beyond `rename_all = "snake_case"` on enums).

export const RoadmapPhaseSchema = z.enum(["exploration", "diagnostic", "syllabus", "negotiation"]);
export const SessionStatusSchema = z.enum(["active", "sealed"]);
export const SkillLevelSchema = z.enum(["novice", "intermediate", "advanced"]);
export const BloomLevelSchema = z.enum(["apply", "analyze", "evaluate", "create"]);
export const BlockTypeSchema = z.enum([
  "prediction_before_reveal",
  "micro_simulator",
  "failure_audit",
  "metacognitive_reflection",
  "concept_check",
]);

export const RevisionEntrySchema = z.object({
  at_turn: z.number(),
  reverted_to_phase: RoadmapPhaseSchema,
  reason: z.string(),
  fields_invalidated: z.array(z.string()),
});

export const LearnerProfileStateSchema = z.object({
  declared_topic: z.string(),
  time_horizon_weeks: z.number(),
  hours_per_week: z.number(),
  self_perceived_level: SkillLevelSchema,
  level_justification: z.string(),
  prior_experience: z.array(z.string()),
  applied_interests: z.array(z.string()),
});

export const KnowledgeGapSchema = z.object({
  concept: z.string(),
  evidence: z.string(),
  is_prerequisite: z.boolean(),
});

export const DiagnosticAssessmentSummarySchema = z.object({
  critical_gaps: z.array(KnowledgeGapSchema),
  confirmed_strengths: z.array(z.string()),
  capstone_options_presented: z.array(z.string()),
  capstone_selected: z.string(),
  difficulty_calibration_note: z.string(),
});

export const MilestoneSchema = z.object({
  id: z.string(),
  title: z.string(),
  bloom_level: BloomLevelSchema,
  learning_objective: z.string(),
  authentic_challenge: z.string(),
  notebook_block_plan: z.array(BlockTypeSchema),
  estimated_hours: z.number(),
});

export const DraftSyllabusTreeSchema = z.object({
  milestones: z.array(MilestoneSchema),
});

export const ApprovalRecordSchema = z.object({
  approved_verbatim_quote: z.string(),
  approved_at_turn: z.number(),
});

// Sealed, final contract handed to the future Notebook Builder agent — the
// one place strict validation genuinely matters, since nothing downstream
// should ever act on an incomplete payload.
export const RoadmapPackageFinalSchema = z.object({
  schema_version: z.string(),
  package_id: z.string(),
  session_id: z.string(),
  generated_at_ms: z.number(),
  status: z.literal("approved"),
  learner_profile: LearnerProfileStateSchema,
  diagnostic_summary: DiagnosticAssessmentSummarySchema,
  syllabus: DraftSyllabusTreeSchema,
  executive_summary: z.string(),
  approval: ApprovalRecordSchema,
  revision_log: z.array(RevisionEntrySchema),
  handoff_notes_for_notebook_builder: z.string(),
});

// While a session is still active, the deliverable buckets accumulate field
// by field across turns (see `RoadmapSession::merge_patch` in Rust) and are
// legitimately incomplete — so they stay loosely typed here. Only
// `final_package`, once sealed, is validated strictly against the schema
// above. Mirrors the same strictness split the Rust backend enforces.
export const RoadmapSessionSchema = z.object({
  session_id: z.string(),
  phase: RoadmapPhaseSchema,
  status: SessionStatusSchema,
  turn_count_in_phase: z.number(),
  dod: z.record(z.string(), z.boolean()),
  clarification_attempts: z.record(z.string(), z.number()),
  learner_profile: z.record(z.string(), z.unknown()),
  diagnostic_summary: z.record(z.string(), z.unknown()),
  syllabus: z.record(z.string(), z.unknown()),
  negotiation: z.record(z.string(), z.unknown()),
  final_package: RoadmapPackageFinalSchema.nullable(),
  revision_log: z.array(RevisionEntrySchema),
  created_at_ms: z.number(),
  updated_at_ms: z.number(),
});

export const RoadmapTurnPayloadSchema = z.object({
  message: z.string(),
  session: RoadmapSessionSchema,
});

export type RoadmapPhase = z.infer<typeof RoadmapPhaseSchema>;
export type RoadmapPackageFinal = z.infer<typeof RoadmapPackageFinalSchema>;
export type RoadmapSession = z.infer<typeof RoadmapSessionSchema>;
export type RoadmapTurnPayload = z.infer<typeof RoadmapTurnPayloadSchema>;
