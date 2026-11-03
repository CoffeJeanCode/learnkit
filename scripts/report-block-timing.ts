// Prints the REAL per-block generation cost from exported dev-trace entries.
//   bun scripts/report-block-timing.ts trace.json
// `trace.json` is a JSON array of dev-trace entries (the `dev://trace` events of
// a session). Nothing here is simulated — compare two runs (before/after a
// change) to see whether it actually made generation faster.
import { readFileSync } from "node:fs";
import { summarizeBlockTiming, timingTable } from "../src/lib/blockTiming";
import type { DevTraceEntry } from "../src/lib/devTrace";

const file = process.argv[2];
if (!file) {
  console.error("uso: bun scripts/report-block-timing.ts <trace.json>");
  process.exit(1);
}
const entries = JSON.parse(readFileSync(file, "utf8")) as DevTraceEntry[];
console.log(timingTable(summarizeBlockTiming(entries)));
