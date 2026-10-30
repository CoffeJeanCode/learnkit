// Asserts the node-style interactions: the drag maths (reorder, slot, snap, wire,
// scrub) and that the chain, the case wiring and the scrub field render with
// their accessible fallbacks. Run with: bun scripts/check-node-interactions.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { dropIndex, isClick, nearRect, reorderTo, slideOffset, snapPort, wirePath } from "../src/features/notebook/blocks/interaction";
import { CaseWiring, ChainOrder } from "../src/features/notebook/blocks/nodes";
import { HOURS_MARKS, markPosition, scrub } from "../src/features/roadmap/panel/panelModel";
import { ScrubField } from "../src/features/roadmap/panel/ScrubField";

// ---- Reorder by dragging ---------------------------------------------------------------
assert.deepEqual(reorderTo(["a", "b", "c", "d"], 3, 1), ["a", "d", "b", "c"], "drop into the middle");
assert.deepEqual(reorderTo(["a", "b", "c"], 0, 2), ["b", "c", "a"], "drop at the end");
assert.deepEqual(reorderTo(["a", "b", "c"], 1, 1), ["a", "b", "c"], "dropped where it was");
assert.deepEqual(reorderTo(["a", "b"], 5, 0), ["a", "b"], "a bad source is ignored");
assert.deepEqual(reorderTo(["a", "b", "c"], 0, 99), ["b", "c", "a"], "past the end clamps");

// centres of four 40px rows, 18px apart
const centres = [20, 78, 136, 194];
assert.equal(dropIndex(centres, 3, 194), 3, "not moved: stays in slot 3");
assert.equal(dropIndex(centres, 3, 60), 1, "pointer between rows 0 and 1: lands at slot 1");
assert.equal(dropIndex(centres, 3, 90), 2, "past row 1's centre: slot 2");
assert.equal(dropIndex(centres, 0, 150), 2, "dragged down past the centres of rows 1 and 2: slot 2");
assert.equal(dropIndex(centres, 2, -50), 0);

// neighbours slide out of the way, nothing else moves
assert.equal(slideOffset(1, 3, 1, 58), 58, "rows between the target and the origin go down");
assert.equal(slideOffset(2, 3, 1, 58), 58);
assert.equal(slideOffset(0, 3, 1, 58), 0);
assert.equal(slideOffset(3, 3, 1, 58), 0, "the dragged row follows the pointer itself");
assert.equal(slideOffset(1, 0, 2, 58), -58, "dragging down slides them up");
assert.equal(slideOffset(3, 0, 2, 58), 0);

// ---- Wires -------------------------------------------------------------------------------
const ports = [
  { id: "true", x: 300, y: 40 },
  { id: "false", x: 300, y: 140 },
];
assert.equal(snapPort(ports, 290, 50, 44)?.id, "true", "close enough snaps");
assert.equal(snapPort(ports, 290, 90, 44), null, "midway between two ports and far from both: no snap");
assert.equal(snapPort(ports, 290, 100, 44)?.id, "false", "within reach of one port: it snaps");
assert.equal(snapPort(ports, 295, 120, 44)?.id, "false", "the nearer port wins");
assert.ok(wirePath(0, 0, 100, 50).startsWith("M 0 0 C "), "a cubic leaving horizontally");
assert.ok(wirePath(0, 0, 100, 50).endsWith("100 50"), "ending on the port");

// ---- Dropping an operator on a socket ------------------------------------------------------
const socketBox = { left: 100, top: 100, right: 220, bottom: 132 };
assert.ok(nearRect(socketBox, 150, 110), "inside the socket");
assert.ok(nearRect(socketBox, 240, 110, 28), "within the magnet margin");
assert.ok(!nearRect(socketBox, 300, 110, 28), "far away");
assert.ok(isClick(2, 1), "a tiny movement is a click");
assert.ok(!isClick(30, 0), "a real movement is a drag");

// ---- Scrubbing a number -------------------------------------------------------------------
const hours = { step: 0.25, min: 0.25, max: 8, pxPerStep: 12 };
assert.equal(scrub(1, 0, hours), 1, "no travel, no change");
assert.equal(scrub(1, 24, hours), 1.5, "two steps right");
assert.equal(scrub(1, -24, hours), 0.5, "two steps left");
assert.equal(scrub(1, 1000, hours), 8, "clamped to the max");
assert.equal(scrub(1, -1000, hours), 0.25, "clamped to the min");
assert.equal(scrub(1, 12, { ...hours, fine: true }), 1.025, "Shift is ten times finer");

// ---- Rendering -------------------------------------------------------------------------------
const noop = () => {};
const chain = renderToStaticMarkup(<ChainOrder steps={["uno", "dos", "tres"]} onChange={noop} render={(s) => <span>{s}</span>} />);
assert.equal((chain.match(/class="chain-item/g) ?? []).length, 3);
assert.ok(chain.includes("Flechas arriba y abajo"), "the keyboard path is announced");
assert.equal((chain.match(/node-port-out/g) ?? []).length, 3, "every node has ports");

const wiring = renderToStaticMarkup(
  <CaseWiring cases={[{ key: "a", content: <span>caso A</span> }, { key: "b", content: <span>caso B</span> }]} verdicts={[true, null]} onChange={noop} />,
);
assert.ok(wiring.includes("Cumple la regla") && wiring.includes("Rompe la regla"), "both sinks");
assert.equal((wiring.match(/class="wire-node"/g) ?? []).length, 2);
assert.equal((wiring.match(/role="radiogroup"/g) ?? []).length, 2, "each case keeps its radio pair for keyboard and screen readers");
assert.ok(wiring.includes('aria-checked="true"'), "the stored verdict shows in the fallback");

const field = renderToStaticMarkup(<ScrubField label="Horas por día" value="1,5" fallback={1} min={0.25} max={8} step={0.25} unit="h" onChange={noop} />);
assert.ok(field.includes('role="spinbutton"') && field.includes('aria-valuenow="1.5"') && field.includes("1,5"));
assert.ok(renderToStaticMarkup(<ScrubField label="x" value="" fallback={1} min={0} max={8} step={1} unit="h" onChange={noop} />).includes("–"), "empty draft shows a dash");

// ---- Legend stops (the MIDI-style scale) ------------------------------------------------------------
assert.equal(markPosition(0.25, 0.25, 8), 0);
assert.equal(markPosition(8, 0.25, 8), 100);
assert.equal(markPosition(99, 0.25, 8), 100, "clamped to the track");
assert.ok(markPosition(4, 0.25, 8) > markPosition(2, 0.25, 8), "marks are ordered along the track");
const legend = renderToStaticMarkup(<ScrubField label="Horas por día" value="2" fallback={1} min={0.25} max={8} step={0.25} unit="h" marks={HOURS_MARKS} onChange={noop} />);
assert.equal((legend.match(/class="scrub-mark(?: is-on)?"/g) ?? []).length, HOURS_MARKS.length, "one legend per stop");
assert.ok(legend.includes("Poner 0,5 h") && legend.includes("scrub-mark is-on"), "stops are labelled and the current one is lit");
assert.equal((legend.match(/is-on/g) ?? []).length, 1, "only the stop that equals the value is lit");

console.log("node interactions: ok");
