// Asserts the capability map's presentation rules: every celebration names the
// evidence rows behind it, and the copy never frames a miss as a penalty.
// Run with: bun scripts/check-capability-map.ts
import assert from "node:assert/strict";
import { celebrationsFor, evidenceDetail, evidenceTitle, nextStepsFor } from "../src/features/capabilities/capabilities";
import type { CapabilityEntry, SkillEvidence } from "../src/lib/tauri";

const DAY = 24 * 60 * 60 * 1000;
const NOW = 10 * DAY;

const ev = (id: string, over: Partial<SkillEvidence> = {}): SkillEvidence => ({
  id,
  skillId: "s",
  courseId: "c",
  blockId: null,
  kind: "practice_gate",
  outcome: "passed",
  attemptNumber: 1,
  hintsShown: 0,
  supportLevel: null,
  rubric: [],
  isTransfer: false,
  createdAtMs: 0,
  ...over,
});

const none = { achieved: false, achievedAtMs: null, evidenceIds: [] as string[] };
const got = (...ids: string[]) => ({ achieved: true, achievedAtMs: 1, evidenceIds: ids });

const entry = (status: Partial<CapabilityEntry["status"]>, over: Partial<CapabilityEntry> = {}): CapabilityEntry => ({
  skillId: "s",
  title: "Pilas",
  objective: null,
  weekNumber: 1,
  milestoneTitle: "Semana 1",
  classComplete: false,
  nextRetrievalAtMs: null,
  evidence: [],
  status: {
    skillId: "s",
    solved: none,
    solvedWithAids: null,
    retained: none,
    retainedAfterDays: null,
    applied: none,
    attemptsRecorded: 0,
    ...status,
  },
  ...over,
});

// Nothing earned -> no celebrations; no reward without evidence.
assert.deepEqual(celebrationsFor(entry({})), []);

// An achievement flagged but with NO evidence ids is never celebrated.
assert.deepEqual(celebrationsFor(entry({ solved: { achieved: true, achievedAtMs: 1, evidenceIds: [] } })), []);

// Solved without aids vs with aids; each cites its rows.
const clean = celebrationsFor(entry({ solved: got("e1"), solvedWithAids: false }));
assert.equal(clean[0].text, "Resolviste el reto sin ayudas.");
assert.deepEqual(clean[0].evidenceIds, ["e1"]);
const aided = celebrationsFor(entry({ solved: got("e1"), solvedWithAids: true }));
assert.match(aided[0].text, /tras reintentar o usar pistas/);

// Retention names the interval, with singular/plural.
const seven = celebrationsFor(entry({ retained: got("e2"), retainedAfterDays: 7 }));
assert.equal(seven[0].text, "Recordaste el concepto 7 días después, sin ver la solución.");
assert.match(celebrationsFor(entry({ retained: got("e2"), retainedAfterDays: 1 }))[0].text, /1 día después/);

// Application, in earned order, every line with its evidence.
const all = celebrationsFor(entry({ solved: got("a"), retained: got("b"), applied: got("c") }));
assert.deepEqual(all.map((c) => c.kind), ["solved", "retained", "applied"]);
assert.ok(all.every((c) => c.evidenceIds.length > 0));

// Next steps: attempted-but-unsolved is encouraging, never a deficit.
const practicing = nextStepsFor(entry({ attemptsRecorded: 2 }), NOW);
assert.equal(practicing.length, 1);
assert.match(practicing[0].text, /no se penaliza/);
assert.equal(nextStepsFor(entry({}), NOW)[0].text, "Aún sin practicar.");

// Solved: retrieval due now / later / not scheduled; applying still open.
const dueNow = nextStepsFor(entry({ solved: got("a") }, { nextRetrievalAtMs: NOW - 1 }), NOW);
assert.match(dueNow[0].text, /Repaso disponible ahora/);
const later = nextStepsFor(entry({ solved: got("a") }, { nextRetrievalAtMs: NOW + DAY }), NOW);
assert.match(later[0].text, /Repaso disponible desde/);
assert.match(nextStepsFor(entry({ solved: got("a") }), NOW)[0].text, /se programará/);
assert.ok(nextStepsFor(entry({ solved: got("a") }), NOW).some((s) => /caso nuevo/.test(s.text)));

// Fully achieved -> no open steps.
assert.deepEqual(nextStepsFor(entry({ solved: got("a"), retained: got("b"), applied: got("c") }), NOW), []);

// Evidence copy: transfer label, escalation framed as a change of approach.
assert.equal(evidenceTitle(ev("t", { isTransfer: true })), "Reto de transferencia — Superado");
assert.equal(evidenceTitle(ev("x", { outcome: "escalated" })), "Práctica — Se continuó con otro enfoque");
assert.ok(!/penal|fall|error/i.test(evidenceTitle(ev("y", { outcome: "failed" }))), "a miss is never framed as a failure");
assert.equal(evidenceDetail(ev("d", { attemptNumber: 2, hintsShown: 1, supportLevel: "faded" })), "intento 2 · 1 ronda de pistas antes · apoyo: atenuado");

console.log("capability map presentation rules: OK");
