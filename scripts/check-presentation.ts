// Asserts the per-block presentation the backend stores (density / emphasis /
// layout): it parses, a missing or malformed value falls back to the neutral
// default, and one bad field never discards the others.
// Run with: bun scripts/check-presentation.ts
import assert from "node:assert/strict";
import { DEFAULT_PRESENTATION, presentationOf } from "../src/lib/schemas";

assert.deepEqual(presentationOf({ presentation: { density: "spacious", emphasis: "visual", layout: "split" } }), { density: "spacious", emphasis: "visual", layout: "split" });
assert.deepEqual(presentationOf({}), DEFAULT_PRESENTATION, "blocks persisted before presentation existed");
assert.deepEqual(presentationOf(null), DEFAULT_PRESENTATION);
assert.deepEqual(presentationOf({ presentation: "nope" }), DEFAULT_PRESENTATION);
assert.deepEqual(
  presentationOf({ presentation: { density: "huge", emphasis: "practice", layout: "split" } }),
  { density: "comfortable", emphasis: "practice", layout: "split" },
  "an unknown value falls back for that field only",
);

console.log("presentation: OK");
