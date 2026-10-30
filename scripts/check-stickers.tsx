// Asserts the sticker peel (geometry after Sticker.js) and the reward sounds' rules, and that the
// sticker renders. Run with: bun scripts/check-stickers.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { MAX_PEEL, SETTLE_MS, SIZE, easeOut, nearestEdge, peelDepth, peelGeometry, settleDepth } from "../src/lib/peel";
import { clackPitch, earnNotes, impactGain, mayPlay } from "../src/lib/sound";
import { PeelSticker } from "../src/features/notebook/rewards/PeelSticker";

// ---- Which edge peels ------------------------------------------------------------------------------
assert.equal(nearestEdge(2, 50), "left");
assert.equal(nearestEdge(97, 40), "right");
assert.equal(nearestEdge(50, 3), "top");
assert.equal(nearestEdge(40, 96), "bottom");

// ---- Depth follows the pointer inward, past a dead zone ------------------------------------------------
assert.equal(peelDepth("left", 3, 50), 0, "entering exactly at the edge lifts nothing");
assert.ok(peelDepth("left", 40, 50) > 0 && peelDepth("left", 60, 50) > peelDepth("left", 40, 50), "deeper pointer, deeper lift");
assert.equal(peelDepth("right", 60, 50), SIZE - 60 - 6, "measured from the right edge for a right peel");
assert.equal(peelDepth("left", 99, 50), SIZE * MAX_PEEL, "never lifts more than the cap");

// ---- The fold sits halfway; what lifts mirrors over it -----------------------------------------------------
const left = peelGeometry("left", 40);
assert.equal(left.fold, 20, "fold halfway between the edge and the pointer");
assert.deepEqual(left.flat, { x: 20, y: 0, w: 80, h: 100 });
assert.deepEqual(left.lifted, { x: 0, y: 0, w: 20, h: 100 });
assert.equal(left.mirror, "translate(40 0) scale(-1 1)", "the flap ends where the pointer is");
const right = peelGeometry("right", 40);
assert.equal(right.fold, 80);
assert.deepEqual(right.flat, { x: 0, y: 0, w: 80, h: 100 });
assert.equal(right.mirror, "translate(160 0) scale(-1 1)");
const top = peelGeometry("top", 40);
assert.deepEqual(top.lifted, { x: 0, y: 0, w: 100, h: 20 });
assert.equal(top.mirror, "translate(0 40) scale(1 -1)");
const bottom = peelGeometry("bottom", 40);
assert.deepEqual(bottom.flat, { x: 0, y: 0, w: 100, h: 80 });
assert.equal(bottom.mirror, "translate(0 160) scale(1 -1)");
for (const edge of ["left", "right", "top", "bottom"] as const) {
  const g = peelGeometry(edge, 0);
  assert.equal(g.depth, 0, `${edge}: flat at rest`);
  assert.equal(g.flat.w * g.flat.h, SIZE * SIZE, `${edge}: nothing is lifted at rest`);
}
assert.equal(peelGeometry("left", 999).depth, SIZE * MAX_PEEL, "geometry is capped too");
// the shade is darkest at the fold and clear at the tip, in the lifted part's own coordinates
assert.deepEqual([left.shade.x1, left.shade.x2], [20, 0]);
assert.deepEqual([right.shade.x1, right.shade.x2], [80, 100]);

// ---- Settling back eases out -----------------------------------------------------------------------------------
assert.equal(easeOut(0), 0);
assert.equal(easeOut(1), 1);
assert.ok(easeOut(0.2) > 0.6, "most of the return happens early (a steep ease-out)");
assert.equal(settleDepth(40, 0), 40);
assert.equal(settleDepth(40, SETTLE_MS), 0);
assert.ok(settleDepth(40, SETTLE_MS / 4) < 40 * 0.25, "a quarter of the way in time, well past a quarter of the way back");

// ---- Sounds say something -----------------------------------------------------------------------------------------
const gold = earnNotes({ shape: "brick", tone: "gold" });
const blue = earnNotes({ shape: "brick", tone: "blue" });
const green = earnNotes({ shape: "brick", tone: "green" });
assert.deepEqual([gold.length, blue.length, green.length], [4, 3, 2], "the sooner it was earned, the more notes");
assert.ok(gold.every((n, i) => i === 0 || n.freq > gold[i - 1]!.freq), "the arpeggio rises");
assert.ok(gold.every((n, i) => i === 0 || n.at > gold[i - 1]!.at), "and the notes follow one another");
assert.equal(earnNotes({ shape: "star", tone: "gold" }).length, 2, "a sticker is a two-tone bell");
assert.equal(impactGain(0.5), 0, "resting pieces make no sound");
assert.equal(impactGain(1.4), 0, "right at the floor: silent");
assert.ok(impactGain(4) > 0 && impactGain(8) > impactGain(4), "harder hit, louder clack");
assert.ok(impactGain(99) <= 1, "never past full volume");
assert.ok(clackPitch(30) > clackPitch(120), "small pieces tick high, big ones knock low");
assert.ok(mayPlay(100, 40, 45) && !mayPlay(100, 70, 45), "rate limit");
assert.ok(mayPlay(0, -Infinity, 1000), "the first one always plays");

// ---- Rendering -----------------------------------------------------------------------------------------------------------
const html = renderToStaticMarkup(<PeelSticker shape="star" tone="gold" size={64} label="Cerraste la clase." />);
assert.ok(html.includes('class="peel-sticker"') && html.includes('aria-label="Cerraste la clase."'), "a labelled sticker");
assert.ok(html.includes('width="64"'), "sized");
assert.ok(!html.includes("scale(-1 1)"), "flat at rest: no folded back yet");
for (const shape of ["star", "bolt", "heart", "flame"] as const) assert.ok(renderToStaticMarkup(<PeelSticker shape={shape} tone="blue" />).includes("<path"), `${shape} draws`);

console.log("stickers: ok");
