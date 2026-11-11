// Pure layout and routing for the `topology_flow` engine — no three.js here,
// so it is testable headless (`scripts/check-scene3d.tsx`). The model never
// writes coordinates: it names nodes and links, and this file decides where
// everything sits and which way each message travels.

export interface TopoNodeLike {
  id: string;
  label: string;
}

export interface TopoLinkLike {
  from: string;
  to: string;
}

export interface TopoMessageLike {
  from: string;
  to: string;
  label: string;
  tone?: string | null;
}

export interface TopoStepLike {
  title: string;
  detail: string;
  messages: TopoMessageLike[];
}

export interface GroundPoint {
  x: number;
  z: number;
}

export interface TopologyLayout {
  positions: Record<string, GroundPoint>;
  /** Half-size of the occupied ground rectangle, for framing the camera. */
  halfX: number;
  halfZ: number;
}

/** Ground distance between neighbouring columns / rows, in scene units. */
export const COLUMN_GAP = 2.9;
export const ROW_GAP = 2.7;

function adjacency(nodes: TopoNodeLike[], links: TopoLinkLike[]): Map<string, string[]> {
  const adj = new Map<string, string[]>(nodes.map((n) => [n.id, []]));
  for (const l of links) {
    if (l.from === l.to || !adj.has(l.from) || !adj.has(l.to)) continue;
    if (!adj.get(l.from)!.includes(l.to)) adj.get(l.from)!.push(l.to);
    if (!adj.get(l.to)!.includes(l.from)) adj.get(l.to)!.push(l.from);
  }
  return adj;
}

/** Layered layout: column = link distance from the first node (the "origin" of
 *  the story), so the flow reads along one axis; inside a column, nodes are
 *  ordered by where their neighbours in the previous column sit, which keeps
 *  links from crossing in the common tree-like cases. */
export function layoutTopology(nodes: TopoNodeLike[], links: TopoLinkLike[]): TopologyLayout {
  const adj = adjacency(nodes, links);
  const column = new Map<string, number>();
  let nextFree = 0;
  // BFS from the first node; any island not reached starts in the next free column.
  for (const start of nodes) {
    if (column.has(start.id)) continue;
    column.set(start.id, nextFree);
    const queue = [start.id];
    let deepest = nextFree;
    while (queue.length) {
      const at = queue.shift()!;
      for (const next of adj.get(at) ?? []) {
        if (column.has(next)) continue;
        const c = column.get(at)! + 1;
        column.set(next, c);
        deepest = Math.max(deepest, c);
        queue.push(next);
      }
    }
    nextFree = deepest + 1;
  }

  const columns: string[][] = [];
  for (const n of nodes) (columns[column.get(n.id)!] ??= []).push(n.id);
  const row = new Map<string, number>();
  columns.forEach((ids, c) => {
    if (c > 0) {
      const bary = (id: string) => {
        const prev = (adj.get(id) ?? []).filter((o) => column.get(o) === c - 1).map((o) => row.get(o)!);
        return prev.length ? prev.reduce((a, b) => a + b, 0) / prev.length : 0;
      };
      const order = new Map(ids.map((id, i) => [id, i]));
      ids.sort((a, b) => bary(a) - bary(b) || order.get(a)! - order.get(b)!);
    }
    ids.forEach((id, i) => row.set(id, i - (ids.length - 1) / 2));
  });

  const positions: Record<string, GroundPoint> = {};
  const midColumn = (columns.length - 1) / 2;
  let halfX = 0;
  let halfZ = 0;
  for (const n of nodes) {
    const x = (column.get(n.id)! - midColumn) * COLUMN_GAP;
    const z = row.get(n.id)! * ROW_GAP;
    positions[n.id] = { x, z };
    halfX = Math.max(halfX, Math.abs(x));
    halfZ = Math.max(halfZ, Math.abs(z));
  }
  return { positions, halfX, halfZ };
}

/** Shortest path of node ids over the (undirected) links, ends included.
 *  `null` when the ends are not connected or unknown. */
export function routeMessage(nodes: TopoNodeLike[], links: TopoLinkLike[], from: string, to: string): string[] | null {
  const adj = adjacency(nodes, links);
  if (!adj.has(from) || !adj.has(to)) return null;
  const parent = new Map<string, string | null>([[from, null]]);
  const queue = [from];
  while (queue.length) {
    const at = queue.shift()!;
    if (at === to) {
      const path: string[] = [];
      for (let cur: string | null = to; cur !== null; cur = parent.get(cur) ?? null) path.unshift(cur);
      return path;
    }
    for (const next of adj.get(at) ?? []) {
      if (parent.has(next)) continue;
      parent.set(next, at);
      queue.push(next);
    }
  }
  return null;
}

/** The route a message is DRAWN along: the link path when there is one, the
 *  straight hop otherwise (a stored visual must never fail to play). */
export function drawableRoute(nodes: TopoNodeLike[], links: TopoLinkLike[], from: string, to: string): string[] {
  return routeMessage(nodes, links, from, to) ?? [from, to];
}

function mermaidText(raw: string): string {
  return raw.replace(/["#;]/g, "'").replace(/\s+/g, " ").trim() || "·";
}

/** The same story as a Mermaid sequence diagram — the "Secuencia" tab, and
 *  the whole visual when WebGL is not available. `upToStep` (1-based) keeps
 *  only the steps already played. */
export function topologyToSequence(nodes: TopoNodeLike[], steps: TopoStepLike[], upToStep?: number): string {
  const lines = ["sequenceDiagram"];
  const alias = new Map(nodes.map((n, i) => [n.id, `p${i}`]));
  for (const n of nodes) lines.push(`  participant ${alias.get(n.id)} as ${mermaidText(n.label)}`);
  const shown = steps.slice(0, upToStep ?? steps.length);
  const first = alias.get(nodes[0]?.id ?? "");
  const last = alias.get(nodes[nodes.length - 1]?.id ?? "");
  shown.forEach((step, i) => {
    if (first && last) lines.push(`  Note over ${first},${last}: ${i + 1}. ${mermaidText(step.title)}`);
    for (const m of step.messages) {
      const a = alias.get(m.from);
      const b = alias.get(m.to);
      if (!a || !b) continue;
      lines.push(`  ${a}${m.tone === "response" ? "-->>" : m.tone === "error" ? "-x" : "->>"}${b}: ${mermaidText(m.label)}`);
    }
  });
  return lines.join("\n");
}

// --- label placement ----------------------------------------------------------
// Labels are HTML pinned next to each icon. On a dense board the obvious spot
// (under the icon) lands on a neighbour, so each label picks, greedily, the
// side where it covers the least of the other icons and labels. Everything is
// measured in "screen units" of the fixed isometric camera of
// `topologyStage.ts`: 1 unit to the right/up on screen.

export type LabelSide = "below" | "right" | "left" | "above";

export interface ScreenBox {
  minR: number;
  maxR: number;
  minU: number;
  maxU: number;
}

const ISO = Math.SQRT1_2;
/** Sine of the camera's elevation (~30°): how much ground depth shows as height. */
const TILT = 0.5;
/** Visible height of one unit of world height. */
export const RISE = 0.865;
/** Rough footprint of any catalogue icon on screen. */
const ICON_HALF_WIDTH = 0.85;
const ICON_HEIGHT = 1.45;
/** Label metrics, assuming the framing the stage aims for (~46 px per unit). */
const CHAR_WIDTH = 0.17;
const LINE_HEIGHT = 0.4;

/** Screen position of a ground point (see the axis mapping in `topologyStage.ts`). */
export function toScreen(p: GroundPoint): { r: number; u: number } {
  return { r: (p.x + p.z) * ISO, u: (p.x - p.z) * ISO * TILT };
}

function iconBox(p: GroundPoint): ScreenBox {
  const { r, u } = toScreen(p);
  return { minR: r - ICON_HALF_WIDTH, maxR: r + ICON_HALF_WIDTH, minU: u - 0.3, maxU: u + ICON_HEIGHT * RISE };
}

function labelBox(p: GroundPoint, side: LabelSide, width: number, height: number): ScreenBox {
  const { r, u } = toScreen(p);
  switch (side) {
    case "below":
      return { minR: r - width / 2, maxR: r + width / 2, minU: u - 0.45 - height, maxU: u - 0.45 };
    case "above":
      return { minR: r - width / 2, maxR: r + width / 2, minU: u + ICON_HEIGHT * RISE + 0.1, maxU: u + ICON_HEIGHT * RISE + 0.1 + height };
    case "right":
      return { minR: r + 1, maxR: r + 1 + width, minU: u + 0.3 - height / 2, maxU: u + 0.3 + height / 2 };
    case "left":
      return { minR: r - 1 - width, maxR: r - 1, minU: u + 0.3 - height / 2, maxU: u + 0.3 + height / 2 };
  }
}

function overlap(a: ScreenBox, b: ScreenBox): number {
  return Math.max(0, Math.min(a.maxR, b.maxR) - Math.max(a.minR, b.minR)) * Math.max(0, Math.min(a.maxU, b.maxU) - Math.max(a.minU, b.minU));
}

const SIDES: LabelSide[] = ["below", "right", "left", "above"];

export function placeLabels(
  nodes: Array<TopoNodeLike & { subtext?: string | null }>,
  layout: TopologyLayout,
): { sides: Record<string, LabelSide>; bounds: ScreenBox } {
  const icons = nodes.map((n) => ({ id: n.id, box: iconBox(layout.positions[n.id]) }));
  const taken: ScreenBox[] = [];
  const sides: Record<string, LabelSide> = {};
  const bounds: ScreenBox = { minR: Infinity, maxR: -Infinity, minU: Infinity, maxU: -Infinity };
  const grow = (b: ScreenBox) => {
    bounds.minR = Math.min(bounds.minR, b.minR);
    bounds.maxR = Math.max(bounds.maxR, b.maxR);
    bounds.minU = Math.min(bounds.minU, b.minU);
    bounds.maxU = Math.max(bounds.maxU, b.maxU);
  };
  for (const n of nodes) {
    const width = Math.max(n.label.length, n.subtext?.length ?? 0) * CHAR_WIDTH + 0.3;
    const height = LINE_HEIGHT * (n.subtext ? 2 : 1) + 0.1;
    let best: LabelSide = "below";
    let bestCost = Infinity;
    SIDES.forEach((side, preference) => {
      const box = labelBox(layout.positions[n.id], side, width, height);
      let cost = preference * 0.02; // ties go to the earlier (more conventional) side
      for (const icon of icons) if (icon.id !== n.id) cost += overlap(box, icon.box);
      for (const other of taken) cost += overlap(box, other);
      if (cost < bestCost) {
        bestCost = cost;
        best = side;
      }
    });
    sides[n.id] = best;
    const chosen = labelBox(layout.positions[n.id], best, width, height);
    taken.push(chosen);
    grow(chosen);
    grow(icons.find((i) => i.id === n.id)!.box);
  }
  if (!nodes.length) return { sides, bounds: { minR: -1, maxR: 1, minU: -1, maxU: 1 } };
  return { sides, bounds };
}
