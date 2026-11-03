// Asserts the notebook rewards: which piece each milestone earns, that one source
// never pays twice, the cap, and a persistence layer that survives corrupt data and
// a store that throws. Run with: bun scripts/check-rewards.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import {
  MAX_PIECES,
  type KeyValueStore,
  type Piece,
  addPiece,
  loadPieces,
  pieceFor,
  pieceSize,
  savePieces,
  spawnX,
  storageKey,
  STICKER_SIZE,
  isBrick,
  isSticker,
  tiltFor,
  toPagePlacement,
  withPage,
  withPlacements,
} from "../src/lib/rewards";
import { useRewards } from "../src/stores/rewards";
import { RewardDockView } from "../src/features/notebook/rewards/RewardDock";

// ---- What each milestone earns ---------------------------------------------------------------
const first = pieceFor({ type: "gate_passed", sourceId: "b1", attempts: 1 }, 1, 0);
assert.deepEqual([first.shape, first.tone, first.cols, first.rows], ["brick", "gold", 2, 2], "first try: the big gold brick");
assert.match(first.why, /a la primera/);
const second = pieceFor({ type: "gate_passed", sourceId: "b2", attempts: 2 }, 1, 0);
assert.deepEqual([second.tone, second.cols, second.rows], ["blue", 2, 1], "second try: a blue 2x1");
const grit = pieceFor({ type: "gate_passed", sourceId: "b3", attempts: 5 }, 1, 0);
assert.deepEqual([grit.tone, grit.cols, grit.rows], ["green", 1, 1], "persistence: a green 1x1");
assert.match(grit.why, /5 intentos/, "says how many tries it took");
assert.ok(pieceSize(first).w > pieceSize(grit).w, "an easier win is a bigger piece");

const stickers = [0, 1, 2, 3, 4].map((n) => pieceFor({ type: "class_closed", sourceId: `c${n}`, }, 1, n).shape);
assert.deepEqual(stickers, ["star", "bolt", "heart", "flame", "star"], "closing a class gives a sticker, cycling through the shapes");
assert.equal(pieceFor({ type: "class_closed", sourceId: "c" }, 1, 0).id, "class:c");

// ---- One source never pays twice, and the table is capped ---------------------------------------
const one = addPiece([], first);
assert.equal(addPiece(one, { ...first, why: "otra" }).length, 1, "the same source cannot be earned again");
let many: Piece[] = [];
for (let i = 0; i < MAX_PIECES + 15; i++) many = addPiece(many, pieceFor({ type: "gate_passed", sourceId: `g${i}`, attempts: 1 }, i, 0));
assert.equal(many.length, MAX_PIECES, "capped");
assert.equal(many[many.length - 1]!.id, `gate:g${MAX_PIECES + 14}`, "the newest are kept");

// ---- Spawning and placements ----------------------------------------------------------------------
const xs = Array.from({ length: 12 }, (_, i) => spawnX(i, 400, 60));
assert.ok(xs.every((x) => x >= 60 && x <= 340), "drops stay inside the walls");
assert.equal(new Set(xs.map((x) => Math.round(x))).size, 12, "consecutive drops land in different places");
const placed = withPlacements(one, [{ id: first.id, x: 0.4, y: 30, angle: 0.1 }, { id: "ghost", x: 1, y: 1, angle: 0 }]);
assert.deepEqual([placed[0]!.x, placed[0]!.y, placed[0]!.angle], [0.4, 30, 0.1]);
assert.equal(placed.length, 1, "unknown ids are ignored");

// ---- Stickers on the page ---------------------------------------------------------------------------------
const sticker = pieceFor({ type: "class_closed", sourceId: "c9" }, 1, 2);
assert.ok(isSticker(sticker) && !isBrick(sticker) && isBrick(first));
const stuckOn = withPage([first, sticker], sticker.id, { x: 0.4, y: 300, rot: 5 });
assert.deepEqual(stuckOn[1]!.page, { x: 0.4, y: 300, rot: 5 });
assert.equal(withPage(stuckOn, first.id, { x: 0.1, y: 1, rot: 0 })[0]!.page, undefined, "a brick is never stuck on the page");
assert.equal(withPage(stuckOn, sticker.id, null)[1]!.page, undefined, "back to the sheet");
const rect = { left: 100, top: 50, width: 800 };
assert.deepEqual(toPagePlacement(500, 350, rect, 4.44), { x: 0.5, y: 300, rot: 4.4 }, "x is a fraction of the page, y px from its top");
assert.equal(toPagePlacement(-999, 0, rect, 0).x, 0.02, "kept inside the page on the left");
assert.equal(toPagePlacement(9999, 0, rect, 0).x, 0.98, "kept inside the page on the right");
assert.equal(toPagePlacement(500, -80, rect, 0).y, 0, "never above the page");
assert.ok(Math.abs(tiltFor("class:c1")) <= 9 && tiltFor("class:c1") === tiltFor("class:c1"), "a small, repeatable tilt");
assert.ok(STICKER_SIZE >= 48);
const kept = loadPieces(
  { getItem: () => JSON.stringify([{ ...sticker, page: { x: 0.2, y: 10, rot: 1 } }, { ...sticker, id: "bad", page: { x: "no" } }]), setItem: () => {} },
  "z",
);
assert.deepEqual(kept.map((p) => p.id), [sticker.id], "a sticker with a malformed page position is dropped");

// ---- Persistence ------------------------------------------------------------------------------------
const memory = new Map<string, string>();
const store: KeyValueStore = { getItem: (k) => memory.get(k) ?? null, setItem: (k, v) => void memory.set(k, v) };
savePieces(store, "nb1", placed);
assert.deepEqual(loadPieces(store, "nb1"), placed, "round trip, positions included");
assert.deepEqual(loadPieces(store, "other"), [], "another notebook has its own table");
memory.set(storageKey("nb1"), "not json");
assert.deepEqual(loadPieces(store, "nb1"), [], "bad JSON gives an empty table");
memory.set(storageKey("nb1"), JSON.stringify([first, { id: 7 }, { ...second, tone: "pink" }, { ...grit, cols: 99 }, "x"]));
assert.deepEqual(loadPieces(store, "nb1").map((p) => p.id), [first.id], "only valid pieces survive");
const throwing: KeyValueStore = {
  getItem: () => {
    throw new Error("blocked");
  },
  setItem: () => {
    throw new Error("blocked");
  },
};
assert.deepEqual(loadPieces(throwing, "nb1"), [], "a store that throws gives an empty table");
savePieces(throwing, "nb1", placed);
assert.deepEqual(loadPieces(null, "nb1"), [], "no storage at all");

// ---- The store: earning opens the table once per source ----------------------------------------------
useRewards.setState({ byNotebook: { n: [] }, open: false, lastEarned: null });
useRewards.getState().earn("n", { type: "gate_passed", sourceId: "b1", attempts: 1 });
assert.equal(useRewards.getState().byNotebook.n!.length, 1);
assert.equal(useRewards.getState().open, true, "a new piece opens the table so it is seen where it drops");
assert.equal(useRewards.getState().lastEarned?.id, "gate:b1");
useRewards.getState().setOpen(false);
assert.equal(useRewards.getState().lastEarned, null, "closing clears the caption");
useRewards.getState().earn("n", { type: "gate_passed", sourceId: "b1", attempts: 1 });
assert.equal(useRewards.getState().byNotebook.n!.length, 1, "passing the same gate again pays nothing");
assert.equal(useRewards.getState().open, false, "and does not reopen the table");

// ---- Rendering --------------------------------------------------------------------------------------------
const noop = () => {};
const view = (open: boolean, caption = "hint", pieces: Piece[] = [first, second]) =>
  renderToStaticMarkup(
    <RewardDockView pieces={pieces} open={open} caption={caption} isNew={false} onOpen={noop} onClose={noop} onSettle={noop} onStick={noop} soundOn onToggleSound={noop} onHover={noop} />,
  );
const closed = view(false);
assert.ok(closed.includes("reward-tab") && closed.includes("Abrir tu mesa de piezas (2)"), "closed: a small tab with the count");
assert.ok(!closed.includes('class="yard"'), "the physics world is not built while closed");
const opened = view(true, "Superaste la compuerta a la primera.");
assert.ok(opened.includes('class="yard"') && opened.includes("2 piezas") && opened.includes("Cerrar tu mesa"), "open: the table, the count and a way to close");
assert.ok(opened.includes("Superaste la compuerta a la primera."), "the caption says how the piece was earned");
assert.ok(opened.includes('aria-label="Tu mesa: 2 piezas"'), "the table is described for screen readers");

// Stickers live on a sheet and on the page, not on the table
const star = pieceFor({ type: "class_closed", sourceId: "c1" }, 1, 0);
const sheetOnly = view(true, "x", [first, star]);
assert.ok(sheetOnly.includes("sticker-sheet") && sheetOnly.includes("Arrástralos a la página"), "an unstuck sticker waits on the sheet");
assert.ok(!view(true, "x", [star]).includes('class="yard"'), "no bricks, no table");
const stuck = view(true, "x", [first, { ...star, page: { x: 0.5, y: 120, rot: 3 } }]);
assert.ok(!stuck.includes("sticker-sheet"), "a sticker already on the page is no longer on the sheet");
assert.ok(opened.includes("Sonido: sí") && view(true).includes('aria-pressed="true"'), "the sound switch is on the panel");

console.log("rewards: ok");
