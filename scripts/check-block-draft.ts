// Asserts how live `notebook_block://delta` events fold into the on-screen draft:
// it only grows within an attempt, resets on a retry, ignores stale events, and
// never invents content. Run with: bun scripts/check-block-draft.ts
import assert from "node:assert/strict";
import { applyBlockDelta, hasDraftContent } from "../src/features/notebook/blockDraft";
import { NotebookBlockEventSchema } from "../src/lib/schemas";

function delta(partial: Record<string, unknown>) {
  return NotebookBlockEventSchema.parse({ event: "delta", document_id: "d1", ...partial });
}

// The wire shape the Rust side emits parses (snake_case, nullable extras).
const wire = NotebookBlockEventSchema.parse({
  event: "delta",
  document_id: "d1",
  attempt: 1,
  preview: { blockType: "anchored_micro_theory", fields: { title: "Rec" } },
  reasoning_chars: null,
  block: null,
});
assert.equal(wire.preview?.fields.title, "Rec");

// Nothing on screen before the first event; non-delta events never create a draft.
assert.equal(applyBlockDelta(null, NotebookBlockEventSchema.parse({ event: "generating", document_id: "d1" })), null);
assert.equal(hasDraftContent(null), false);

// Text grows within an attempt.
let draft = applyBlockDelta(null, delta({ attempt: 1, preview: { blockType: "anchored_micro_theory", fields: { title: "Rec" } } }));
assert.deepEqual(draft?.fields, { title: "Rec" });
draft = applyBlockDelta(draft, delta({ attempt: 1, preview: { blockType: "anchored_micro_theory", fields: { title: "Recursión", systemRule: "Una" } } }));
assert.deepEqual(draft?.fields, { title: "Recursión", systemRule: "Una" });
assert.equal(draft?.blockType, "anchored_micro_theory");
assert.equal(hasDraftContent(draft), true);

// A preview that does not know the type yet never erases the one we have.
draft = applyBlockDelta(draft, delta({ attempt: 1, preview: { blockType: null, fields: { title: "Recursión", systemRule: "Una regla" } } }));
assert.equal(draft?.blockType, "anchored_micro_theory");

// Reasoning only counts up, and never touches the text.
draft = applyBlockDelta(draft, delta({ attempt: 1, reasoning_chars: 900 }));
assert.equal(draft?.reasoningChars, 900);
draft = applyBlockDelta(draft, delta({ attempt: 1, reasoning_chars: 400 }));
assert.equal(draft?.reasoningChars, 900, "an out-of-order smaller count must not rewind");
assert.equal(draft?.fields.title, "Recursión");

// A stale attempt is ignored; a newer one starts from a clean draft.
const before = draft;
assert.equal(applyBlockDelta(draft, delta({ attempt: 0, preview: { blockType: "x", fields: { title: "viejo" } } })), before);
draft = applyBlockDelta(draft, delta({ attempt: 2, reasoning_chars: 10 }));
assert.deepEqual(draft, { attempt: 2, blockType: null, fields: {}, reasoningChars: 10 }, "a retry discards the rejected draft");

// Reasoning alone is already something to show (slow reasoning models).
assert.equal(hasDraftContent(draft), true);
assert.equal(hasDraftContent({ attempt: 1, blockType: null, fields: {}, reasoningChars: 0 }), false);

console.log("block draft rules: OK");
