// Asserts the study switch only ever changes PRESENTATION, and that an unknown
// version never leaks the gamified UI. Run with: bun scripts/check-study-presentation.ts
import assert from "node:assert/strict";
import { presentationFor } from "../src/features/study/presentation";

assert.deepEqual(presentationFor("gamified"), { showCapabilityMap: true, showTransferFraming: true });
assert.deepEqual(presentationFor("plain"), { showCapabilityMap: false, showTransferFraming: false });
// Not loaded yet / unreadable settings: plain, so nothing gamified can flash for a `plain` participant.
assert.deepEqual(presentationFor(null), presentationFor("plain"));
// The surface the experiment may touch is exactly these two flags.
assert.deepEqual(Object.keys(presentationFor("gamified")).sort(), ["showCapabilityMap", "showTransferFraming"]);

console.log("study presentation rules: OK");
