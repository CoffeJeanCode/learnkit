// Asserts the developer-mode trace grouping: tool calls attach to the agent run
// whose time window contains them, the ring buffer is bounded/idempotent, and
// the "problems" filter keeps failures, retries and fail-opens.
// Run with: bun scripts/check-dev-trace.ts
import assert from "node:assert/strict";
import {
  RING_LIMIT,
  appendEntry,
  buildTimeline,
  filterTimeline,
  isProblem,
  type DevTraceEntry,
} from "../src/lib/devTrace";

const e = (id: number, tsMs: number, kind: string, extra: Partial<DevTraceEntry> = {}): DevTraceEntry => ({ id, tsMs, kind, ...extra });

// A run recorded at t=10_000 that lasted 4s owns the tool call at t=8_000, not the one at t=20_000.
const entries = [
  e(1, 8_000, "tool_call", { tool: "publish_notebook_block" }),
  e(2, 10_000, "agent_run", { durationMs: 4_000, ok: true, flow: "notebook_block" }),
  e(3, 20_000, "tool_call", { tool: "orphan" }),
];
const timeline = buildTimeline(entries);
assert.equal(timeline.length, 2, "owned tool call is nested, orphan stays standalone");
assert.deepEqual(timeline[0].tools.map((t) => t.id), [1]);
assert.equal(timeline[1].entry.id, 3);

// Ring buffer: bounded, drops oldest, ignores duplicate ids (snapshot + live overlap).
let ring: DevTraceEntry[] = [];
for (let i = 1; i <= RING_LIMIT + 5; i++) ring = appendEntry(ring, e(i, i, "retry"));
assert.equal(ring.length, RING_LIMIT);
assert.equal(ring[0].id, 6);
assert.equal(appendEntry(ring, e(RING_LIMIT + 5, 0, "retry")), ring);

// Problems filter.
assert.equal(isProblem(e(1, 1, "agent_run", { ok: false, error: "x" })), true);
assert.equal(isProblem(e(1, 1, "retry")), true);
assert.equal(isProblem(e(1, 1, "critique", { verdict: "llm_unavailable_fail_open" })), true);
assert.equal(isProblem(e(1, 1, "critique", { verdict: "llm_accepted" })), false);
assert.equal(isProblem(e(1, 1, "agent_run", { ok: true })), false);
const mixed = buildTimeline([e(1, 1, "agent_run", { ok: true }), e(2, 2, "agent_run", { ok: false, error: "boom" })]);
assert.deepEqual(filterTimeline(mixed, { onlyProblems: true, scope: null }).map((i) => i.entry.id), [2]);

// Scope filter keeps unscoped entries (agent runs have none) and hides other classes.
const scoped = buildTimeline([e(1, 1, "critique", { scope: "a" }), e(2, 2, "critique", { scope: "b" }), e(3, 3, "agent_run")]);
assert.deepEqual(filterTimeline(scoped, { onlyProblems: false, scope: "a" }).map((i) => i.entry.id), [1, 3]);

console.log("dev trace grouping: OK");
