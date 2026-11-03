// Asserts the real block-timing baseline: the dev-trace rows the backend writes
// parse, malformed rows are skipped, and the summary (mean / percentiles /
// retry and backstop rates) is right. Run with: bun scripts/check-block-timing.ts
//
// To measure a real session: export the dev-trace entries to a JSON array and run
//   bun scripts/report-block-timing.ts trace.json
import assert from "node:assert/strict";
import { parseBlockTiming, percentile, summarizeBlockTiming, timingTable } from "../src/lib/blockTiming";
import type { DevTraceEntry } from "../src/lib/devTrace";

const t = (id: number, type: string, ms: number, attempts: number, source = "model"): DevTraceEntry => ({
  id,
  tsMs: id,
  kind: "note",
  verdict: "block_generation_timing",
  notes: [`position=${id}`, `block_type=${type}`, `source=${source}`, `attempts=${attempts}`, `elapsed_ms=${ms}`, "critic_deferred=true"],
});

assert.deepEqual(parseBlockTiming(t(1, "hands_on_mission", 9000, 2)), {
  position: 1,
  blockType: "hands_on_mission",
  source: "model",
  attempts: 2,
  elapsedMs: 9000,
  criticDeferred: true,
});
assert.equal(parseBlockTiming({ id: 2, tsMs: 2, kind: "note", verdict: "other" }), null, "not a timing row");
assert.equal(parseBlockTiming({ ...t(3, "x", 1, 1), notes: ["block_type=x", "elapsed_ms=oops", "attempts=1"] }), null, "malformed rows are skipped");

assert.equal(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 50), 5);
assert.equal(percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 90), 9);
assert.equal(percentile([], 90), 0);

const rows = summarizeBlockTiming([
  t(1, "anchored_micro_theory", 4000, 1),
  t(2, "anchored_micro_theory", 6000, 2),
  t(3, "declarative_visual_diagram", 20000, 3, "backstop"),
  t(4, "interactive_prediction_gate", 3000, 1),
  { id: 9, tsMs: 9, kind: "retry" },
]);
assert.deepEqual(rows.map((r) => r.blockType), ["declarative_visual_diagram", "anchored_micro_theory", "interactive_prediction_gate", "ALL"], "slowest first, ALL last");
const theory = rows.find((r) => r.blockType === "anchored_micro_theory")!;
assert.equal(theory.count, 2);
assert.equal(theory.meanMs, 5000);
assert.equal(theory.retryRate, 0.5);
assert.equal(rows.find((r) => r.blockType === "declarative_visual_diagram")!.backstopRate, 1);
const all = rows.at(-1)!;
assert.equal(all.count, 4);
assert.equal(all.meanAttempts, 1.75);

assert.match(timingTable(rows), /\| ALL \| 4 \|/);
assert.equal(summarizeBlockTiming([]).length, 0);

console.log("block timing: OK");
