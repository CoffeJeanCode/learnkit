// Pure helpers behind the notebook's interactive mechanics (cloze micro-steps,
// causal ordering, "bug bounty" line picking). Kept free of React so
// `scripts/check-rendering.tsx` can exercise them directly.

/** Case/space-insensitive form used to compare a typed cloze token. */
export const normalizeToken = (s: string): string => s.trim().toLowerCase().replace(/\s+/g, " ");

export const tokenMatches = (typed: string, expected: string): boolean => normalizeToken(typed) === normalizeToken(expected);

/** A copy of `items` with the element at `index` moved one slot up (-1) or
 *  down (+1). Out-of-range moves return the list unchanged. */
export function moveItem<T>(items: readonly T[], index: number, delta: -1 | 1): T[] {
  const target = index + delta;
  const next = [...items];
  if (index < 0 || index >= next.length || target < 0 || target >= next.length) return next;
  [next[index], next[target]] = [next[target]!, next[index]!];
  return next;
}

/** Stable key for an answer, to remember attempts that already failed so the
 *  same guess can't be submitted twice. */
export const answerKey = (parts: readonly (string | boolean | null)[]): string => JSON.stringify(parts);

/** Lines of a flawed snippet, 1-based in the UI. A trailing newline does not
 *  add a phantom empty line (the backend counts lines the same way). */
export function splitSnippetLines(snippet: string): string[] {
  const lines = snippet.replace(/\r\n/g, "\n").split("\n");
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}

// --- Node-style drag: reorder a chain, wire a case to a verdict -----------------------------------
// The vocabulary comes from node editors (TouchDesigner): items are nodes with
// ports, you drag a node and its neighbours slide out of the way, you drag a
// WIRE from an output port and it snaps to an input port when close enough.

/** Moves the item at `from` so it ends up at index `to` (insertion, like dropping a node into a chain). */
export function reorderTo<T>(items: readonly T[], from: number, to: number): T[] {
  const next = [...items];
  if (from < 0 || from >= next.length) return next;
  const [item] = next.splice(from, 1);
  next.splice(Math.min(next.length, Math.max(0, to)), 0, item!);
  return next;
}

/** The slot a dragged item would land in: how many OTHER items have their centre above the pointer. */
export function dropIndex(centres: readonly number[], from: number, pointerY: number): number {
  let index = 0;
  centres.forEach((c, i) => {
    if (i !== from && c < pointerY) index++;
  });
  return index;
}

/** Vertical shift of item `i` while `from` is dragged over slot `to`: neighbours slide one `slot` out of its way. */
export function slideOffset(i: number, from: number, to: number, slot: number): number {
  if (i === from) return 0;
  if (from < to && i > from && i <= to) return -slot;
  if (from > to && i >= to && i < from) return slot;
  return 0;
}

export interface Port {
  id: string;
  x: number;
  y: number;
}

/** The closest port within `radius` of the pointer, or null: the wire "snaps" only when close. */
export function snapPort(ports: readonly Port[], x: number, y: number, radius: number): Port | null {
  let best: Port | null = null;
  let bestDistance = radius;
  for (const p of ports) {
    const d = Math.hypot(p.x - x, p.y - y);
    if (d <= bestDistance) {
      bestDistance = d;
      best = p;
    }
  }
  return best;
}

/** A cubic wire leaving the output port horizontally and entering the input port horizontally. */
export function wirePath(x1: number, y1: number, x2: number, y2: number): string {
  const reach = Math.max(36, Math.abs(x2 - x1) * 0.5);
  const f = (n: number) => Math.round(n * 10) / 10;
  return `M ${f(x1)} ${f(y1)} C ${f(x1 + reach)} ${f(y1)}, ${f(x2 - reach)} ${f(y2)}, ${f(x2)} ${f(y2)}`;
}

/** Whether a point is inside (or within `pad` px of) a rectangle: the socket's "magnet" zone. */
export function nearRect(rect: { left: number; top: number; right: number; bottom: number }, x: number, y: number, pad = 0): boolean {
  return x >= rect.left - pad && x <= rect.right + pad && y >= rect.top - pad && y <= rect.bottom + pad;
}

/** A press that barely moved is a click (the keyboard/mouse path), not a drag. */
export function isClick(dx: number, dy: number, tolerance = 5): boolean {
  return Math.hypot(dx, dy) < tolerance;
}
