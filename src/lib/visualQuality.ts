// Objective quality metrics for a generated visual — the measurable half of the
// "no overlapping labels / readable on dark / anchored" rules the generator is
// told in prose. Used by `scripts/eval-block-models.ts` and the check scripts,
// so a change to prompts or schemas can be judged by numbers, not by eye.
// Pure (no React/DOM): text boxes are ESTIMATED from font size, which is enough
// to catch the collisions that matter (two labels on top of each other).

import { mermaidTypeOf } from "./mermaidTypes";
import { MAX_LAYERS, isLayer, overlapDepth, placeParts } from "./scene3d/partPlacement";
import { solidIssues } from "./scene3d/solidBuild";
import { surfaceIssues } from "./scene3d/surfaceBuild";
import { buildGroup, parseElement } from "./algebra";
import { GUITAR_CHORDS } from "./instrument";
import { GRID_COLS, GRID_ROWS } from "./illustration";
import { stateAt, valenceIssues } from "./molecule";
import { routeMessage } from "./scene3d/topologyLayout";
import { isPlaceholderLabel } from "./sequenceStrip";
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
        if (isPlaceholderLabel(n.label)) issues.push(`nodo "${n.id}" con etiqueta genérica "${n.label}"`);
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
    case "topology_flow": {
      const ids = new Set(visual.nodes.map((n) => n.id));
      if (ids.size !== visual.nodes.length) issues.push("ids de nodo repetidos");
      for (const l of visual.links) for (const end of [l.from, l.to]) if (!ids.has(end)) issues.push(`link a "${end}" que no es un nodo`);
      for (const step of visual.steps) {
        for (const m of step.messages) {
          if (!ids.has(m.from) || !ids.has(m.to)) issues.push(`mensaje "${m.label}" entre nodos que no existen`);
          else if (!routeMessage(visual.nodes, visual.links, m.from, m.to)) issues.push(`mensaje "${m.label}" sin camino de links entre ${m.from} y ${m.to}`);
        }
      }
      if (visual.nodes.length > 8) issues.push(`${visual.nodes.length} nodos (máximo 8)`);
      if (!visual.steps.length) issues.push("sin pasos");
      stats.elements = visual.nodes.length + visual.links.length;
      break;
    }
    case "explorable_model": {
      const ids = new Set(visual.parts.map((p) => p.id));
      if (ids.size !== visual.parts.length) issues.push("ids de parte repetidos");
      for (const p of visual.parts) if (!p.explanation.trim()) issues.push(`parte "${p.label}" sin explicación`);
      if (visual.parts.filter((p) => p.zone === "center").length > 1) issues.push("más de una parte en el centro");
      if (visual.parts.length > 10) issues.push(`${visual.parts.length} partes (máximo 10)`);
      const layers = visual.parts.filter(isLayer).length;
      if (layers > MAX_LAYERS) issues.push(`${layers} capas (máximo ${MAX_LAYERS})`);
      const depth = overlapDepth(placeParts(visual.parts, visual.shell, !!visual.shell?.cutaway));
      if (depth > 0.25) {
        stats.overlaps++;
        issues.push("las partes no caben sin solaparse: reduce tamaños o cantidades");
      }
      stats.elements = visual.parts.length;
      break;
    }
    case "mermaid": {
      if (!visual.code.trim()) issues.push("mermaid sin código");
      // The header is what Mermaid renders by: it must match the declared type.
      const declared = mermaidTypeOf(visual.code);
      if (declared && declared !== visual.chartType) issues.push(`chartType ${visual.chartType} pero el código abre como ${declared}`);
      stats.elements = visual.code.split("\n").length;
      break;
    }
    case "molecule_3d": {
      const atoms = visual.atoms.map((a) => ({ ...a, charge: a.charge ?? 0 }));
      const steps = visual.steps.map((st) => ({ ...st, form: st.form as Array<[string, string]>, break: st.break as Array<[string, string]> }));
      for (let k = 0; k <= steps.length; k++) {
        const st = stateAt(atoms, visual.bonds, steps, k);
        const bad = valenceIssues(atoms, st.bonds, st.charges);
        if (bad.length) issues.push(`${k === 0 ? "al inicio" : `tras el paso ${k}`}: ${bad.join("; ")}`);
      }
      if (visual.atoms.length > 40) issues.push(`${visual.atoms.length} átomos (máximo 40)`);
      stats.elements = visual.atoms.length;
      break;
    }
    case "algebraic_structure": {
      const g = buildGroup(visual.group.kind, visual.group.n, visual.subgroup);
      if (!g) issues.push("grupo fuera de rango (máximo 24 elementos)");
      for (const s of visual.subgroup) if (!parseElement(visual.group.kind, visual.group.n, s)) issues.push(`"${s}" no es un elemento del grupo`);
      stats.elements = g?.elements.length ?? 0;
      break;
    }
    case "instrument":
      for (const c of [...visual.chords, ...visual.progression.map((p) => p.chord)]) if (!GUITAR_CHORDS[c]) issues.push(`acorde "${c}" fuera del diccionario`);
      if (visual.strum && !/^[DU-]{4,16}$/.test(visual.strum)) issues.push(`rasgueo "${visual.strum}" inválido`);
      stats.elements = visual.chords.length;
      break;
    case "illustration_kit": {
      const ids = new Set(visual.items.map((i) => i.id));
      const slots = new Set<string>();
      for (const it of visual.items) {
        if (it.slot[0] >= GRID_COLS || it.slot[1] >= GRID_ROWS) issues.push(`"${it.id}" fuera de la cuadrícula`);
        const key = it.slot.join(",");
        if (slots.has(key) && it.kind !== "ground") issues.push(`dos piezas en la casilla ${key}`);
        slots.add(key);
      }
      for (const l of visual.links) if (!ids.has(l.from) || !ids.has(l.to)) issues.push(`enlace ${l.from}→${l.to} a piezas inexistentes`);
      for (const f of visual.forces) if (!ids.has(f.on)) issues.push(`fuerza "${f.label}" sobre una pieza inexistente`);
      stats.elements = visual.items.length;
      break;
    }
    case "code_figure":
      if (!visual.code.includes("fig.")) issues.push("el código no usa el SDK fig");
      if (/\b(fetch|XMLHttpRequest|WebSocket|eval|localStorage|sessionStorage|indexedDB|importScripts)\b|\bparent\.|\btop\.|window\.open|postMessage|new\s+Function/i.test(visual.code)) issues.push("el código usa una API prohibida");
      stats.elements = 1;
      break;
    case "surface_3d":
      issues.push(...surfaceIssues(visual));
      for (const p of visual.paths) if (!p.explanation.trim()) issues.push(`trayectoria "${p.label}" sin explicación`);
      stats.elements = visual.paths.length + visual.markers.length + 1;
      break;
    case "solid_model":
      issues.push(...solidIssues(visual.features));
      stats.elements = visual.features.length;
      break;
    case "sequence_strip": {
      const first = visual.rows[0]?.chunks.length ?? 0;
      if (visual.rows.length > 3) issues.push(`${visual.rows.length} filas (máximo 3)`);
      for (const row of visual.rows) {
        if (row.chunks.length > 12) issues.push(`fila "${row.label}" con ${row.chunks.length} trozos (máximo 12)`);
        if (row.chunks.length !== first) issues.push(`fila "${row.label}" no tiene los mismos trozos que la primera`);
        else if (row !== visual.rows[0]) {
          const changed = row.chunks.filter((c, k) => c.text.trim() !== visual.rows[0].chunks[k].text.trim()).length;
          if (changed * 2 > row.chunks.length + 1) issues.push(`fila "${row.label}" cambia ${changed} de ${row.chunks.length} trozos: son pasos, no una variante`);
        }
        for (const c of row.chunks) {
          if (!c.meaning.trim() || isPlaceholderLabel(c.meaning)) issues.push(`trozo "${c.text}" sin significado concreto`);
          if (!c.text.trim() || c.text.length > 16) issues.push(`trozo "${c.text}" con texto vacío o de más de 16 caracteres`);
        }
      }
      stats.elements = visual.rows.reduce((n, r) => n + r.chunks.length, 0);
      break;
    }
    case "conceptual_matrix":
      if (visual.rows.some((r) => r.length !== visual.headers.length)) issues.push("fila con distinto número de columnas que los encabezados");
      stats.elements = visual.rows.length;
      break;
  }
  return { engine: visual.renderEngine, issues, score: Math.max(0, Number((1 - issues.length * 0.15).toFixed(2))), stats };
}
