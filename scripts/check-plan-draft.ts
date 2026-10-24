// Asserts how the plan draft grows as the syllabus is written in parts.
// Run with: bun scripts/check-plan-draft.ts
import assert from "node:assert/strict";
import { applyPlanPart, draftProgress } from "../src/features/roadmap/planDraft";
import { PlanPartEventSchema } from "../src/lib/schemas";

const SID = "s1";
const outline = PlanPartEventSchema.parse({
  kind: "outline",
  sessionId: SID,
  courseTitle: "Linux desde cero",
  coreFocus: "Moverte con la terminal",
  weeks: [
    { week: 2, title: "Archivos", weeklyGoal: "Crear y mover archivos" },
    { week: 1, title: "Primeros pasos", weeklyGoal: null },
  ],
  capstoneTitle: "Mi servidor",
});
const week = (n: number, sid = SID) =>
  PlanPartEventSchema.parse({ kind: "week", sessionId: sid, week: n, sessions: [{ label: "Días 1-2 — Abrir la terminal", hours: 2.5, objective: "Abrir una terminal" }] });

// The wire format the backend sends parses (camelCase, optional fields).
assert.equal(PlanPartEventSchema.safeParse({ kind: "started", sessionId: SID }).success, true);
assert.equal(PlanPartEventSchema.safeParse({ kind: "outline", sessionId: SID }).success, false, "an incomplete outline is refused");
assert.equal(PlanPartEventSchema.safeParse({ kind: "nope", sessionId: SID }).success, false);
assert.equal(
  PlanPartEventSchema.safeParse({ kind: "week", sessionId: SID, week: 1, sessions: [{ label: "x", hours: 1 }] }).success,
  true,
  "an objective is optional",
);

// The outline creates the draft in week order, every week waiting for its sessions.
let draft = applyPlanPart(null, outline, SID);
assert.deepEqual(draft?.weeks.map((w) => [w.week, w.title, w.sessions]), [[1, "Primeros pasos", null], [2, "Archivos", null]]);
assert.equal(draft?.weeks[0].weeklyGoal, null);
assert.deepEqual(draftProgress(draft!), { ready: 0, total: 2 });

// A week fills only itself, in whatever order they finish.
draft = applyPlanPart(draft, week(2), SID);
assert.equal(draft?.weeks[0].sessions, null, "week 1 is still waiting");
assert.equal(draft?.weeks[1].sessions?.[0].hours, 2.5);
assert.deepEqual(draftProgress(draft!), { ready: 1, total: 2 });
draft = applyPlanPart(draft, week(1), SID);
assert.deepEqual(draftProgress(draft!), { ready: 2, total: 2 });

// Nothing invents or corrupts a plan.
assert.equal(applyPlanPart(null, week(1), SID), null, "a week before any outline is ignored");
assert.deepEqual(applyPlanPart(draft, week(9), SID), draft, "an unknown week is ignored");
assert.deepEqual(applyPlanPart(draft, week(1, "other"), SID), draft, "another session's event is ignored");
assert.equal(applyPlanPart(draft, outline, "other"), draft, "so is its outline");

// A new attempt wipes what was shown.
assert.equal(applyPlanPart(draft, PlanPartEventSchema.parse({ kind: "started", sessionId: SID }), SID), null);
assert.equal(applyPlanPart(draft, PlanPartEventSchema.parse({ kind: "started", sessionId: "other" }), SID), draft);

console.log("plan draft rules: OK");
