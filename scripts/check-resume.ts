// Asserts "where I left it": where opening a session lands, and that the saved
// position survives corrupt data and a store that throws. Run with: bun scripts/check-resume.ts
import assert from "node:assert/strict";
import { type KeyValueStore, landing, loadResume, resumeKey, saveResume } from "../src/lib/resume";

const sealed = { status: "sealed", imported_course_id: "course1" };
const building = { status: "active", imported_course_id: null };

// ---- Landing --------------------------------------------------------------------------------
assert.deepEqual(landing(null, null), { view: "roadmap" }, "no session: the landing");
assert.deepEqual(landing(building, { view: "notebook", classId: "c1" }), { view: "roadmap" }, "a session still being built opens on its conversation");
assert.deepEqual(landing(sealed, null), { view: "plan" }, "sealed with nothing saved: the plan that is waiting");
assert.deepEqual(landing(sealed, { view: "plan" }), { view: "plan" });
assert.deepEqual(landing(sealed, { view: "roadmap" }), { view: "roadmap" }, "left on the conversation without class: back to roadmap");
assert.deepEqual(
  landing(sealed, { view: "notebook", classId: "c2", blockId: "b7", blockClassId: "c2" }),
  { view: "notebook", courseId: "course1", classId: "c2", blockId: "b7" },
  "left in a notebook: that class and that block",
);
assert.deepEqual(
  landing(sealed, { view: "notebook", classId: "c2", blockId: "b7", blockClassId: "c1" }),
  { view: "notebook", courseId: "course1", classId: "c2", blockId: undefined },
  "a block remembered from ANOTHER class is not scrolled to",
);
assert.deepEqual(landing(sealed, { view: "notebook" }), { view: "plan" }, "no class saved: the plan");
assert.deepEqual(landing({ status: "sealed", imported_course_id: null }, { view: "notebook", classId: "c1" }), { view: "plan" }, "no course to open: the plan");

// ---- Persistence ------------------------------------------------------------------------------
const memory = new Map<string, string>();
const store: KeyValueStore = { getItem: (k) => memory.get(k) ?? null, setItem: (k, v) => void memory.set(k, v) };
assert.equal(loadResume(store, "s1"), null, "nothing saved yet");
saveResume(store, "s1", { view: "notebook", classId: "c1" });
saveResume(store, "s1", { blockId: "b3", blockClassId: "c1" });
assert.deepEqual(loadResume(store, "s1"), { view: "notebook", classId: "c1", blockId: "b3", blockClassId: "c1" }, "patches merge");
saveResume(store, "s1", { view: "plan" });
assert.equal(loadResume(store, "s1")?.classId, "c1", "leaving the notebook keeps the class and block for the way back");
assert.equal(loadResume(store, "s2"), null, "each session has its own position");

// ---- Continuous Learning Position Tracking ----------------------------------------------------
// Re-entering notebook at class c2, block b5 updates both the class and block
saveResume(store, "s1", { view: "notebook", classId: "c2", blockId: "b5", blockClassId: "c2" });
const updated = loadResume(store, "s1");
assert.equal(updated?.view, "notebook");
assert.equal(updated?.classId, "c2");
assert.equal(updated?.blockId, "b5");
assert.equal(updated?.blockClassId, "c2");

// Landing accurately directs back to class c2 and block b5
assert.deepEqual(
  landing(sealed, updated),
  { view: "notebook", courseId: "course1", classId: "c2", blockId: "b5" },
  "reopening session resumes at last class and block",
);

// If the session is sealed and user had viewed roadmap, saved class/block is preserved
saveResume(store, "s1", { view: "roadmap" });
const preserved = loadResume(store, "s1");
assert.equal(preserved?.classId, "c2", "class is preserved across roadmap view");
assert.equal(preserved?.blockId, "b5", "block is preserved across roadmap view");

memory.set(resumeKey("s1"), "not json");
assert.equal(loadResume(store, "s1"), null, "bad JSON gives nothing");
memory.set(resumeKey("s1"), JSON.stringify({ view: "chat" }));
assert.equal(loadResume(store, "s1"), null, "an unknown view is rejected");
memory.set(resumeKey("s1"), JSON.stringify({ view: "notebook", classId: 7 }));
assert.equal(loadResume(store, "s1"), null, "a wrong type is rejected");
const throwing: KeyValueStore = {
  getItem: () => {
    throw new Error("blocked");
  },
  setItem: () => {
    throw new Error("blocked");
  },
};
assert.equal(loadResume(throwing, "s1"), null, "a store that throws gives nothing");
saveResume(throwing, "s1", { view: "plan" });
assert.equal(loadResume(null, "s1"), null, "no storage at all");

console.log("resume: ok");
