// Objective quality metrics for a generated visual — the measurable half of the
// "no overlapping labels / readable on dark / anchored" rules the generator is
// told in prose. Used by `scripts/eval-block-models.ts` and the check scripts,
// so a change to prompts or schemas can be judged by numbers, not by eye.
// Pure (no React/DOM): text boxes are ESTIMATED from font size, which is enough
// to catch the collisions that matter (two labels on top of each other).

import type { StaticVisualSpec } from "./schemas";

export interface VisualQuality {
  engine: string;
  /** Human-readable problems, one per finding. */
  issues: string[];
  /** 1 = clean; each issue costs 0.15, floor 0. */
  score: number;
  stats: { elements: number; texts: number; overlaps: number; unbackedTexts: number; kebabProps: number; offPalette: number };
}

/** Text/stroke colours that vanish on the dark canvas (the prompt forbids them for key elements). */
const LOW_CONTRAST = new Set(["#0f172a", "#1e293b", "#1e3a8a", "#14532d", "#000", "#000000"]);

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

const num = (v: unknown, d = 0) => (typeof v === "number" && Number.isFinite(v) ? v : typeof v === "string" && v.trim() !== "" && Number.isFinite(Number(v)) ? Number(v) : d);

/** Estimated bounding box of an SVG `text` element. */
export function textBox(props: Record<string, unknown>, label: string): Box {
  const size = num(props.fontSize, 14);
  const w = label.length * size * 0.56;
  const h = size * 1.2;
  const x = num(props.x);
  const y = num(props.y);
  const anchor = props.textAnchor;
  const x0 = anchor === "middle" ? x - w / 2 : anchor === "end" ? x - w : x;
  return { x0, y0: y - h * 0.8, x1: x0 + w, y1: y + h * 0.2 };
}

function overlapArea(a: Box, b: Box): number {
  const w = Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0);
  const h = Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0);
  return w > 0 && h > 0 ? w * h : 0;
}

type El = { tag: string; props: Record<string, unknown>; label?: string | null };

function svgIssues(elements: El[], where: string, stats: VisualQuality["stats"], issues: string[]) {
  const boxes: { box: Box; label: string }[] = [];
  elements.forEach((el, i) => {
    stats.elements++;
    for (const key of Object.keys(el.props)) {
      if (key.includes("-") && !key.startsWith("data-") && !key.startsWith("aria-")) {
        stats.kebabProps++;
        issues.push(`${where}[${i}]: prop "${key}" en kebab-case`);
      }
    }
    for (const key of ["fill", "stroke"] as const) {
      const c = el.props[key];
      if (typeof c === "string" && LOW_CONTRAST.has(c.toLowerCase()) && (el.tag === "text" || key === "stroke")) {
        stats.offPalette++;
        issues.push(`${where}[${i}]: ${key} ${c} casi no se ve sobre el lienzo oscuro`);
      }
    }
    if (el.tag === "text") {
      stats.texts++;
      if (!(i > 0 && elements[i - 1].tag === "rect")) {
        stats.unbackedTexts++;
        issues.push(`${where}[${i}]: texto "${el.label ?? ""}" sin rect de fondo`);
      }
      boxes.push({ box: textBox(el.props, el.label ?? ""), label: el.label ?? "" });
    }
  });
  for (let a = 0; a < boxes.length; a++) {
    for (let b = a + 1; b < boxes.length; b++) {
      const area = overlapArea(boxes[a].box, boxes[b].box);
      const smaller = Math.min((boxes[a].box.x1 - boxes[a].box.x0) * (boxes[a].box.y1 - boxes[a].box.y0), (boxes[b].box.x1 - boxes[b].box.x0) * (boxes[b].box.y1 - boxes[b].box.y0));
      if (smaller > 0 && area / smaller > 0.15) {
        stats.overlaps++;
        issues.push(`${where}: "${boxes[a].label}" y "${boxes[b].label}" se solapan`);
      }
    }
  }
}

export function scoreVisual(visual: StaticVisualSpec): VisualQuality {
  const stats = { elements: 0, texts: 0, overlaps: 0, unbackedTexts: 0, kebabProps: 0, offPalette: 0 };
  const issues: string[] = [];
  switch (visual.renderEngine) {
    case "declarative_svg":
      svgIssues(visual.elements as El[], "elements", stats, issues);
      for (const g of visual.groups) svgIssues(g.elements as El[], `groups[${g.groupId}]`, stats, issues);
      if (stats.elements > 60) issues.push(`${stats.elements} elementos: demasiado para un solo modelo mental`);
      break;
    case "diagram_spec": {
      const ids = new Set<string>();
      for (const n of visual.nodes) {
        if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(n.id)) issues.push(`nodo con id inválido "${n.id}"`);
        if (ids.has(n.id)) issues.push(`id de nodo repetido "${n.id}"`);
        ids.add(n.id);
      }
      for (const e of visual.edges) for (const end of [e.from, e.to]) if (!ids.has(end)) issues.push(`arista a "${end}" que no es un nodo`);
      if (visual.nodes.length > 10) issues.push(`${visual.nodes.length} nodos (máximo 10)`);
      stats.elements = visual.nodes.length + visual.edges.length;
      break;
    }
    case "interactive_module":
      stats.elements = 1; // validity is enforced by the backend (domain::interactive_module)
      break;
    case "mermaid":
      if (!visual.code.trim()) issues.push("mermaid sin código");
      stats.elements = visual.code.split("\n").length;
      break;
    case "conceptual_matrix":
      if (visual.rows.some((r) => r.length !== visual.headers.length)) issues.push("fila con distinto número de columnas que los encabezados");
      stats.elements = visual.rows.length;
      break;
  }
  return { engine: visual.renderEngine, issues, score: Math.max(0, Number((1 - issues.length * 0.15).toFixed(2))), stats };
}
