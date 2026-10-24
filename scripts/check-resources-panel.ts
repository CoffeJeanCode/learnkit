// Asserts what the "Recursos" panel shows for each research state and when
// opening it may start the research. Run with: bun scripts/check-resources-panel.ts
import assert from "node:assert/strict";
import { KIND_LABEL, panelView, shouldAutoStart } from "../src/features/notebook/resourcesPanelModel";
import { ClassResourceSchema, CourseResearchSchema, ResearchStatusEventSchema, SearchKeyStatusSchema } from "../src/lib/schemas";
import type { ClassResource, CourseResearch } from "../src/lib/schemas";

const resource: ClassResource = { title: "Guía", url: "https://a.dev", kind: "tutorial", why: "w", whatToDo: "d" };
const research = (status: CourseResearch["status"], detail: string | null = null): CourseResearch => ({ status, detail });

// Resources win over every research state.
for (const status of [null, "pending", "running", "done", "skipped_no_key", "failed"] as const) {
  assert.deepEqual(panelView(research(status), [resource]), { kind: "list", resources: [resource] }, `list wins over ${status}`);
}
assert.equal(panelView(null, [resource]).kind, "list", "even before the status loads");

// Without resources the state decides.
assert.equal(panelView(null, []).kind, "searching", "before the first read");
for (const status of [null, "pending", "running"] as const) assert.equal(panelView(research(status), []).kind, "searching", `${status}`);
assert.equal(panelView(research("skipped_no_key"), []).kind, "needs_key");
assert.deepEqual(panelView(research("failed", "sin conexión"), []), { kind: "failed", detail: "sin conexión" });
assert.equal(panelView(research("done"), []).kind, "none");

// Auto-start: only when it cannot waste work or hide a failure.
assert.equal(shouldAutoStart(null, true), false, "never before the status is known");
assert.equal(shouldAutoStart(research(null), false), true, "a course that predates the feature starts once");
assert.equal(shouldAutoStart(research("running"), true), false);
assert.equal(shouldAutoStart(research("pending"), true), false);
assert.equal(shouldAutoStart(research("done"), true), false, "a finished research is never repeated");
assert.equal(shouldAutoStart(research("failed"), true), false, "a failure waits for the student's retry");
assert.equal(shouldAutoStart(research("skipped_no_key"), false), false, "no key yet: nothing to attempt");
assert.equal(shouldAutoStart(research("skipped_no_key"), true), true, "a key appeared: try again");

// Every kind has a student-facing label.
for (const kind of ["tutorial", "documentation", "video", "exercise", "reference"] as const) assert.ok(KIND_LABEL[kind], kind);

// The wire shapes the Rust side emits parse (camelCase resources, snake_case event).
assert.equal(ClassResourceSchema.parse({ title: "T", url: "https://a.dev", kind: "video", why: "w", whatToDo: "d" }).whatToDo, "d");
assert.equal(CourseResearchSchema.parse({ status: null, detail: null }).status, null);
assert.equal(CourseResearchSchema.parse({ status: "skipped_no_key", detail: null }).status, "skipped_no_key");
assert.equal(SearchKeyStatusSchema.parse({ configured: true, source: "env" }).source, "env");
assert.equal(ResearchStatusEventSchema.parse({ course_id: "c1", status: "done", detail: "3 clases con recursos" }).course_id, "c1");

console.log("resources panel rules: OK");
