// Layout of the `illustration_kit` engine: grid slots → pixels, link routes,
// force arrows. Pure, so the checks can verify that nothing overlaps. Mirror of
// the vocabularies in `src-tauri/src/domain/illustration.rs`.

export const GRID_COLS = 6;
export const GRID_ROWS = 4;
export const CELL_W = 125;
export const CELL_H = 100;
export const PAD = 10;
export const VIEW_W = PAD * 2 + GRID_COLS * CELL_W;
// Extra room under the last row for downward forces (weights) to show.
export const VIEW_H = PAD * 2 + GRID_ROWS * CELL_H + 70;

export interface Item {
  id: string;
  kind: string;
  label: string;
  slot: [number, number];
  value?: string | null;
  angle?: number | null;
}

/** Centre of a slot in viewBox pixels. */
export function slotCentre([c, r]: [number, number]): [number, number] {
  return [PAD + c * CELL_W + CELL_W / 2, PAD + r * CELL_H + CELL_H / 2];
}

/** Unit vector (screen: y down) for a force direction word or angle in degrees (0° = right, 90° = up). */
export function directionVector(direction: string): [number, number] {
  const words: Record<string, number> = { right: 0, "up-right": 45, up: 90, "up-left": 135, left: 180, "down-left": 225, down: 270, "down-right": 315 };
  const deg = words[direction] ?? Number(direction);
  const rad = ((Number.isFinite(deg) ? deg : 270) * Math.PI) / 180;
  return [Math.cos(rad), -Math.sin(rad)];
}

/** A link path between two slots: straight for ropes/rods/arrows, a zigzag
 *  for springs, an orthogonal (L-shaped) route for wires. */
export function linkPath(kind: string, a: [number, number], b: [number, number]): string {
  const [x1, y1] = a;
  const [x2, y2] = b;
  if (kind === "wire") {
    return `M${x1},${y1} L${x2},${y1} L${x2},${y2}`;
  }
  if (kind === "spring") {
    const n = 10;
    const dx = (x2 - x1) / n;
    const dy = (y2 - y1) / n;
    const len = Math.hypot(x2 - x1, y2 - y1) || 1;
    const [px, py] = [(-(y2 - y1) / len) * 9, ((x2 - x1) / len) * 9];
    const pts = [`M${x1},${y1}`];
    for (let i = 1; i < n; i++) pts.push(`L${x1 + dx * i + (i % 2 ? px : -px)},${y1 + dy * i + (i % 2 ? py : -py)}`);
    pts.push(`L${x2},${y2}`);
    return pts.join(" ");
  }
  return `M${x1},${y1} L${x2},${y2}`;
}

/** Trim a segment so it starts/ends at the edge of the pieces, not their centre. */
export function trimmed(a: [number, number], b: [number, number], by = 26): [[number, number], [number, number]] {
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
  const ux = (b[0] - a[0]) / len;
  const uy = (b[1] - a[1]) / len;
  const cut = Math.min(by, len / 3);
  return [
    [a[0] + ux * cut, a[1] + uy * cut],
    [b[0] - ux * cut, b[1] - uy * cut],
  ];
}

/** A force arrow that leaves the piece clear of its label and never leaves
 *  the drawing: a downward force starts below the label, and every arrow is
 *  shortened to fit inside the view. */
export function forceArrow(item: Item, direction: string, magnitude: number) {
  const [cx, cy] = slotCentre(item.slot);
  const [ux, uy] = directionVector(direction);
  const box = labelBox(item);
  const startGap = uy > 0.5 ? box.y + box.h + 4 - cy : 24;
  const x1 = cx + ux * Math.max(24, Math.abs(ux) > 0.5 ? 28 : 0);
  const y1 = cy + uy * startGap;
  let len = 22 + 14 * Math.min(3, Math.max(1, magnitude));
  const room = (v: number, lo: number, hi: number, u: number) => (u > 0 ? (hi - v) / u : u < 0 ? (lo - v) / u : Infinity);
  const margin = 20;
  len = Math.max(12, Math.min(len, room(x1, margin, VIEW_W - margin, ux), room(y1, margin, VIEW_H - margin, uy)));
  const x2 = x1 + ux * len;
  const y2 = y1 + uy * len;
  // The name goes beside the tip for vertical arrows, past it otherwise.
  const vertical = Math.abs(uy) > Math.abs(ux);
  return { x1, y1, x2, y2, lx: vertical ? x2 + 18 : x2 + ux * 14, ly: vertical ? y2 - uy * 6 + 4 : y2 + 4 };
}

/** Label boxes (estimated) never overlap: each item's label sits under its slot. */
export function labelBox(item: Item): { x: number; y: number; w: number; h: number } {
  const [cx, cy] = slotCentre(item.slot);
  const text = item.value ? `${item.label} · ${item.value}` : item.label;
  const w = Math.min(CELL_W - 6, 7 * text.length + 12);
  return { x: cx - w / 2, y: cy + 30, w, h: 18 };
}
