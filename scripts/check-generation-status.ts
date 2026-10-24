// Asserts what the student is told while the next block is generated: only real
// signals, retries called out, the block type narrated, and no internal jargon.
// Run with: bun scripts/check-generation-status.ts
import assert from "node:assert/strict";
import type { BlockDraft } from "../src/features/notebook/blockDraft";
import { BLOCK_TYPE_LABEL, BLOCK_TYPE_PURPOSE, skeletonKindFor } from "../src/features/notebook/blockTypeInfo";
import { describeGeneration } from "../src/features/notebook/generationStatus";

const draft = (over: Partial<BlockDraft> = {}): BlockDraft => ({ attempt: 1, blockType: null, fields: {}, reasoningChars: 0, ...over });

// Nothing has happened yet.
assert.equal(describeGeneration("preparando", null).phase, "preparing");
assert.equal(describeGeneration("preparando", null).headline, "Preparando el siguiente paso…");
assert.equal(describeGeneration("preparando", draft()).phase, "preparing", "an empty draft says nothing new");

// The generator started but nothing has arrived.
assert.equal(describeGeneration("escribiendo", null).phase, "writing");

// Reasoning only (slow reasoning models): say so, with no block type to claim.
const thinking = describeGeneration("escribiendo", draft({ reasoningChars: 1200 }));
assert.equal(thinking.phase, "thinking");
assert.match(thinking.headline, /Pensando/);

// The block type is known: narrate what is being made and what it asks.
const arming = describeGeneration("escribiendo", draft({ blockType: "interactive_prediction_gate", reasoningChars: 50 }));
assert.equal(arming.phase, "writing");
assert.equal(arming.headline, "Armando: Predicción");
assert.equal(arming.detail, BLOCK_TYPE_PURPOSE.interactive_prediction_gate);
assert.equal(arming.skeleton, "choices");

// Once text streams in, the verb changes from arming to writing.
const writing = describeGeneration("escribiendo", draft({ blockType: "anchored_micro_theory", fields: { title: "Recursión" } }));
assert.equal(writing.headline, "Escribiendo: Explicación");
assert.equal(writing.skeleton, "text");

// A retry is called out, by the generator's stage OR by the draft's attempt, and wins over everything.
for (const [stage, d] of [
  ["reintentando", null],
  ["escribiendo", draft({ attempt: 2, blockType: "anchored_micro_theory" })],
] as const) {
  const retry = describeGeneration(stage, d);
  assert.equal(retry.phase, "retrying");
  assert.match(retry.headline, /Ajustando/);
}

// Every block type has a label, a purpose and a skeleton; none leaks an identifier or jargon.
const TYPES = Object.keys(BLOCK_TYPE_LABEL);
assert.equal(TYPES.length, 8, "all 8 block types are described");
for (const type of TYPES) {
  assert.ok(BLOCK_TYPE_PURPOSE[type], `${type} has a purpose line`);
  assert.ok(["text", "choices", "diagram", "task"].includes(skeletonKindFor(type)), `${type} has a skeleton`);
  for (const text of [BLOCK_TYPE_LABEL[type], BLOCK_TYPE_PURPOSE[type]]) {
    assert.ok(!/_/.test(text), `no internal identifier in "${text}"`);
    assert.ok(!/\b(modelo|IA|LLM|agente|prompt)\b/i.test(text), `no jargon in "${text}"`);
  }
}
for (const status of [thinking, arming, writing, describeGeneration("preparando", null)]) {
  assert.ok(!/\b(modelo|IA|LLM|agente|prompt)\b/i.test(`${status.headline} ${status.detail}`), "student-facing copy has no jargon");
  assert.ok(!/\d+\s?%|\bETA\b|segundos? restantes/i.test(`${status.headline} ${status.detail}`), "never a fake percentage or ETA");
}

// Unknown or missing type falls back to the neutral silhouette.
assert.equal(skeletonKindFor(null), "text");
assert.equal(skeletonKindFor("something_new"), "text");

console.log("generation status rules: OK");
