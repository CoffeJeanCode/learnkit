// Pure logic behind the student's knowledge graph: a deterministic layout and
// the helpers the forms need. No React, no Tauri — `scripts/check-concept-graph.ts`
// exercises it directly. Nothing here is generated: every concept and relation
// comes from the student.
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";

export const NODE_W = 220;
export const CLASS_H = 46;
export const CONCEPT_H = 34;
// Wide enough for a relation's label to sit in the gap between two columns.
const COL_GAP = 170;
const ROW_GAP = 8;
const CLASS_GAP = 16;
const PAD = 24;
/** Extra room right of the last column for labels of same-column relations. */
const SIDE_ROOM = 150;
const HEADER_H = 34;

/** Quick-pick wording for a relation. Shortcuts for typing only — the student
 *  may write anything, and a relation is never created without their text. */
export const RELATION_HINTS = ["causa", "es parte de", "contrasta con", "depende de", "es un ejemplo de", "me recuerda a"] as const;

export type NodeKind = "class" | "concept";

export interface GraphNode {
  id: string;
  kind: NodeKind;
  label: string;
  /** The class this node belongs to (itself, for a class). */
  classId: string;
  /** Which week column the node sits in. */
  column: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface GraphEdge {
  id: string;
  fromId: string;
  toId: string;
  label: string;
  /** SVG path between the two nodes. */
  path: string;
  labelX: number;
  labelY: number;
  /** `start` for relations inside one column (the label sits right of the
   *  boxes); `middle` for the ones that cross the gap between columns. */
  labelAnchor: "start" | "middle";
}

export interface GraphColumn {
  x: number;
  label: string;
}

export interface GraphLayout {
  nodes: GraphNode[];
  edges: GraphEdge[];
  columns: GraphColumn[];
  width: number;
  height: number;
}

export function truncate(text: string, max: number): string {
  const t = text.trim();
  return t.length <= max ? t : `${t.slice(0, Math.max(1, max - 1)).trimEnd()}…`;
}

/** "Semana 2: Panorama" -> "Semana 2" (class titles are composed that way at
 *  import time); anything else has no week header. */
export function weekLabel(title: string): string {
  const m = /^(Semana\s+\d+)\s*:/i.exec(title.trim());
  return m ? m[1]! : "";
}

/** A class title without its "Semana N:" prefix — what the node shows. */
export function classLabel(title: string): string {
  return title.replace(/^Semana\s+\d+\s*:\s*/i, "").trim() || title.trim();
}

/** Lays classes out as one column per milestone (week), a class box followed
 *  by the concepts the student wrote about it. Edges are curves between node
 *  borders; relations whose ends no longer exist are dropped, never drawn. */
export function layoutGraph(classes: ClassRecord[], concepts: Concept[], links: ConceptLink[]): GraphLayout {
  const ordered = [...classes].sort((a, b) => a.order_index - b.order_index);
  const groups: ClassRecord[][] = [];
  const groupOf = new Map<string, number>();
  for (const c of ordered) {
    let g = groupOf.get(c.milestone_id);
    if (g === undefined) {
      g = groups.length;
      groupOf.set(c.milestone_id, g);
      groups.push([]);
    }
    groups[g]!.push(c);
  }

  const nodes: GraphNode[] = [];
  const columns: GraphColumn[] = [];
  let bottom = PAD + HEADER_H;
  groups.forEach((group, col) => {
    const x = PAD + col * (NODE_W + COL_GAP);
    columns.push({ x, label: weekLabel(group[0]!.title) });
    let y = PAD + HEADER_H;
    for (const c of group) {
      nodes.push({ id: c.id, kind: "class", label: classLabel(c.title), classId: c.id, column: col, x, y, w: NODE_W, h: CLASS_H });
      y += CLASS_H + ROW_GAP;
      for (const k of concepts.filter((k) => k.classId === c.id)) {
        nodes.push({ id: k.id, kind: "concept", label: k.label, classId: c.id, column: col, x: x + 16, y, w: NODE_W - 16, h: CONCEPT_H });
        y += CONCEPT_H + ROW_GAP;
      }
      y += CLASS_GAP - ROW_GAP;
    }
    bottom = Math.max(bottom, y);
  });

  const byId = new Map(nodes.map((n) => [n.id, n]));
  const edges: GraphEdge[] = [];
  for (const l of links) {
    const a = byId.get(l.fromId);
    const b = byId.get(l.toId);
    if (!a || !b) continue;
    edges.push(edgeBetween(l, a, b));
  }
  return { nodes, edges, columns, width: PAD * 2 + Math.max(1, groups.length) * (NODE_W + COL_GAP) - COL_GAP + SIDE_ROOM, height: bottom + PAD };
}

function edgeBetween(link: ConceptLink, a: GraphNode, b: GraphNode): GraphEdge {
  const ay = a.y + a.h / 2;
  const by = b.y + b.h / 2;
  const sameColumn = a.column === b.column;
  // Always leave by the border that faces the other node; inside one column
  // both ends use the right border and the curve bulges outwards.
  let x1: number, x2: number, c1x: number, c2x: number;
  if (sameColumn) {
    x1 = a.x + a.w;
    x2 = b.x + b.w;
    c1x = Math.max(x1, x2) + 56;
    c2x = c1x;
  } else if (a.column < b.column) {
    x1 = a.x + a.w;
    x2 = b.x;
    const dx = Math.max(40, (x2 - x1) / 2);
    c1x = x1 + dx;
    c2x = x2 - dx;
  } else {
    x1 = a.x;
    x2 = b.x + b.w;
    const dx = Math.max(40, (x1 - x2) / 2);
    c1x = x1 - dx;
    c2x = x2 + dx;
  }
  // Midpoint of the cubic Bézier (t = 0.5), where the label sits.
  const mx = (x1 + 3 * c1x + 3 * c2x + x2) / 8;
  const labelY = (ay + 3 * ay + 3 * by + by) / 8;
  return {
    id: link.id,
    fromId: link.fromId,
    toId: link.toId,
    label: link.label,
    path: `M ${x1} ${ay} C ${c1x} ${ay}, ${c2x} ${by}, ${x2} ${by}`,
    labelX: sameColumn ? mx + 6 : mx,
    labelY: sameColumn ? labelY + 4 : labelY - 6,
    labelAnchor: sameColumn ? "start" : "middle",
  };
}

export interface NodeOption {
  id: string;
  kind: NodeKind;
  label: string;
}

/** Every node a relation can connect, in reading order (a class, then its
 *  concepts) — what the "desde"/"hacia" selects offer. */
export function nodeOptions(classes: ClassRecord[], concepts: Concept[]): NodeOption[] {
  const out: NodeOption[] = [];
  for (const c of [...classes].sort((a, b) => a.order_index - b.order_index)) {
    out.push({ id: c.id, kind: "class", label: classLabel(c.title) });
    for (const k of concepts.filter((k) => k.classId === c.id)) out.push({ id: k.id, kind: "concept", label: k.label });
  }
  return out;
}

/** What the student can connect a class's new idea TO: every EARLIER class and
 *  the concepts they wrote there — the "relate what you just learned with what
 *  you already knew" step. */
export function earlierNodes(classes: ClassRecord[], concepts: Concept[], classId: string): NodeOption[] {
  const current = classes.find((c) => c.id === classId);
  if (!current) return [];
  const earlier = classes.filter((c) => c.order_index < current.order_index);
  return nodeOptions(earlier, concepts);
}

/** "A —relación→ B", the plain-text form of a relation (list view, tooltips). */
export function describeLink(link: ConceptLink, options: NodeOption[]): string {
  const name = (id: string) => options.find((o) => o.id === id)?.label ?? "(borrado)";
  return `${name(link.fromId)} —${link.label}→ ${name(link.toId)}`;
}
