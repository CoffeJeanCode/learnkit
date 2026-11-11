// Pure geometry of the `solid_model` engine (a CAD-like part built feature by
// feature), free of three.js so the checks and the React legend can use it.
// Mirrors `src-tauri/src/domain/solid_model.rs` — same conventions, same
// messages' intent:
// - CAD frame, Z up. Plane `xy`: (u, v, n) -> (x, y, z); `xz`: u = x, v = z,
//   normal +Y; `yz`: u = y, v = z, normal +X.
// - `extrude` grows from `offset` along +normal by `depth`.
// - `cut` / `hole` with a `depth` go INTO the part, from `offset` towards
//   -normal; without one they cut through everything.
// - `revolve` spins the profile around the plane's v axis (u = 0).

import { isPlaceholderLabel } from "../sequenceStrip";
import type { SolidFeature } from "../schemas";

export type Vec3 = [number, number, number];
export interface Box3 {
  min: Vec3;
  max: Vec3;
}

export const MAX_SOLID_FEATURES = 6;
export const MAX_SOLID_DIM = 1000;
export const MAX_SOLID_RATIO = 50;
export const MAX_POLYGON_POINTS = 12;
const EPS = 1e-6;

export const isAdditive = (op: string) => op === "extrude" || op === "revolve";

/** Axis indices (x=0, y=1, z=2) of the plane's u, v and normal. */
export function planeAxes(plane: string): [number, number, number] {
  if (plane === "xz") return [0, 2, 1];
  if (plane === "yz") return [1, 2, 0];
  return [0, 1, 2];
}

/** The profile's outline on its plane, `at` included, as [u, v] points
 *  (counter-clockwise; circles sampled). */
export function profileOutline(f: SolidFeature, segments = 32): Array<[number, number]> {
  const [au, av] = f.at;
  const p = f.profile;
  if (p.kind === "rect" && p.w && p.h) {
    const [hw, hh] = [p.w / 2, p.h / 2];
    return [
      [au - hw, av - hh],
      [au + hw, av - hh],
      [au + hw, av + hh],
      [au - hw, av + hh],
    ];
  }
  if (p.kind === "circle" && p.d) {
    const r = p.d / 2;
    return Array.from({ length: segments }, (_, i) => {
      const t = (i / segments) * Math.PI * 2;
      return [au + r * Math.cos(t), av + r * Math.sin(t)] as [number, number];
    });
  }
  if (p.kind === "polygon" && p.points.length >= 3) {
    const pts = p.points.map(([u, v]) => [u + au, v + av] as [number, number]);
    // Counter-clockwise, whatever order the model wrote them in.
    return signedArea(pts) < 0 ? pts.reverse() : pts;
  }
  return [];
}

export function signedArea(points: Array<[number, number]>): number {
  let a = 0;
  for (let i = 0; i < points.length; i++) {
    const [x1, y1] = points[i];
    const [x2, y2] = points[(i + 1) % points.length];
    a += x1 * y2 - x2 * y1;
  }
  return a / 2;
}

/** (u0, u1, v0, v1) of the profile on its plane, or null if it is incomplete. */
export function profileExtent(f: SolidFeature): [number, number, number, number] | null {
  const [au, av] = f.at;
  const p = f.profile;
  if (p.kind === "rect") return p.w && p.h ? [au - p.w / 2, au + p.w / 2, av - p.h / 2, av + p.h / 2] : null;
  if (p.kind === "circle") return p.d ? [au - p.d / 2, au + p.d / 2, av - p.d / 2, av + p.d / 2] : null;
  if (p.points.length === 0) return null;
  const us = p.points.map((q) => q[0] + au);
  const vs = p.points.map((q) => q[1] + av);
  return [Math.min(...us), Math.max(...us), Math.min(...vs), Math.max(...vs)];
}

/** Range along the normal the feature occupies. */
export function normalRange(f: SolidFeature, through: number): [number, number] {
  if (f.op === "extrude") return [f.offset, f.offset + (f.depth ?? 0)];
  if (f.op === "revolve") {
    const e = profileExtent(f);
    const r = e ? Math.max(Math.abs(e[0]), Math.abs(e[1])) : 0;
    return [f.offset - r, f.offset + r];
  }
  return f.depth != null ? [f.offset - f.depth, f.offset] : [-through, through];
}

export function featureBox(f: SolidFeature, through: number): Box3 | null {
  const e = profileExtent(f);
  if (!e) return null;
  let [u0, u1] = [e[0], e[1]];
  if (f.op === "revolve") {
    const r = Math.max(Math.abs(u0), Math.abs(u1));
    [u0, u1] = [-r, r];
  }
  const [n0, n1] = normalRange(f, through);
  const [ui, vi, ni] = planeAxes(f.plane);
  const min: Vec3 = [0, 0, 0];
  const max: Vec3 = [0, 0, 0];
  for (const [axis, lo, hi] of [
    [ui, u0, u1],
    [vi, e[2], e[3]],
    [ni, n0, n1],
  ] as const) {
    min[axis] = Math.min(lo, hi);
    max[axis] = Math.max(lo, hi);
  }
  return { min, max };
}

const union = (a: Box3, b: Box3): Box3 => ({
  min: [0, 1, 2].map((k) => Math.min(a.min[k], b.min[k])) as Vec3,
  max: [0, 1, 2].map((k) => Math.max(a.max[k], b.max[k])) as Vec3,
});
const touches = (a: Box3, b: Box3) => [0, 1, 2].every((k) => a.min[k] <= b.max[k] + EPS && b.min[k] <= a.max[k] + EPS);
const overlaps = (a: Box3, b: Box3) => [0, 1, 2].every((k) => a.min[k] < b.max[k] - EPS && b.min[k] < a.max[k] - EPS);

/** Box of the material (additive features only) — what the camera frames. */
export function partBounds(features: SolidFeature[]): Box3 | null {
  let box: Box3 | null = null;
  for (const f of features) {
    if (!isAdditive(f.op)) continue;
    const b = featureBox(f, MAX_SOLID_DIM * 4);
    if (b) box = box ? union(box, b) : b;
  }
  return box;
}

function segmentsCross(a: [number, number], b: [number, number], c: [number, number], d: [number, number]): boolean {
  const orient = (p: [number, number], q: [number, number], r: [number, number]) => (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
  return orient(a, b, c) * orient(a, b, d) < -EPS && orient(c, d, a) * orient(c, d, b) < -EPS;
}

export function polygonIsSimple(points: Array<[number, number]>): boolean {
  const n = points.length;
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      if (j === i + 1 || (i === 0 && j === n - 1)) continue;
      if (segmentsCross(points[i], points[(i + 1) % n], points[j], points[(j + 1) % n])) return false;
    }
  }
  return true;
}

/** Everything wrong with a part, in the learner-safe words `visualQuality` uses. */
export function solidIssues(features: SolidFeature[]): string[] {
  const issues: string[] = [];
  if (features.length < 1 || features.length > MAX_SOLID_FEATURES) return [`${features.length} operaciones (1-${MAX_SOLID_FEATURES})`];
  if (!isAdditive(features[0].op)) issues.push("la primera operación no crea material");
  const sizes: number[] = [];
  for (const f of features) {
    if (!f.label.trim() || isPlaceholderLabel(f.label)) issues.push(`operación "${f.id}" sin etiqueta concreta`);
    if (!f.explanation.trim()) issues.push(`operación "${f.label}" sin explicación`);
    const p = f.profile;
    const dims = p.kind === "rect" ? [p.w, p.h] : p.kind === "circle" ? [p.d] : [];
    if (dims.some((d) => d == null)) {
      issues.push(`perfil ${p.kind} de "${f.label}" incompleto`);
      continue;
    }
    if (p.kind === "polygon" && (p.points.length < 3 || p.points.length > MAX_POLYGON_POINTS || !polygonIsSimple(p.points))) {
      issues.push(`polígono de "${f.label}" inválido`);
      continue;
    }
    if (f.op === "extrude" && f.depth == null) issues.push(`extrusión "${f.label}" sin profundidad`);
    if (f.op === "hole" && p.kind !== "circle") issues.push(`agujero "${f.label}" no circular`);
    const e = profileExtent(f);
    if (f.op === "revolve" && e && e[0] < -EPS) issues.push(`revolución "${f.label}" cruza su eje`);
    // A revolve spins 360°: any depth on it is not a measure (Rust's repair drops it).
    const measures = [...(dims as number[]), ...(p.kind === "polygon" && e ? [e[1] - e[0], e[3] - e[2]] : []), ...(f.depth != null && f.op !== "revolve" ? [f.depth] : [])];
    if (measures.some((m) => !(m > 0) || m > MAX_SOLID_DIM)) {
      issues.push(`medidas de "${f.label}" fuera de rango`);
      continue;
    }
    sizes.push(...measures);
  }
  if (issues.length) return issues;
  if (sizes.length && Math.max(...sizes) / Math.min(...sizes) > MAX_SOLID_RATIO) issues.push(`medidas desproporcionadas (más de ${MAX_SOLID_RATIO}:1)`);
  let part: Box3 | null = null;
  for (const f of features) {
    const b = featureBox(f, MAX_SOLID_DIM * 4);
    if (!b) continue;
    if (isAdditive(f.op)) {
      if (part && !touches(part, b)) issues.push(`"${f.label}" flota separada de la pieza`);
      part = part ? union(part, b) : b;
    } else if (part) {
      if (!overlaps(part, b)) issues.push(`"${f.label}" no toca la pieza`);
      else {
        const [ui, vi] = planeAxes(f.plane);
        const covers = (k: number) => b.min[k] <= part!.min[k] + EPS && b.max[k] >= part!.max[k] - EPS;
        if (f.depth == null && covers(ui) && covers(vi)) issues.push(`"${f.label}" borraría la pieza entera`);
      }
    }
  }
  return issues;
}

// --- views -------------------------------------------------------------------

export const VIEW_LABEL: Record<string, string> = { iso: "Isométrica", front: "Frontal", top: "Superior", right: "Derecha" };

/** Where the camera looks FROM (unit vector, CAD frame) and which way is up. */
export function viewDirection(view: string): { dir: Vec3; up: Vec3 } {
  switch (view) {
    case "front":
      return { dir: [0, -1, 0], up: [0, 0, 1] };
    case "top":
      return { dir: [0, 0, 1], up: [0, 1, 0] };
    case "right":
      return { dir: [1, 0, 0], up: [0, 0, 1] };
    default: {
      const s = 1 / Math.sqrt(3);
      return { dir: [s, -s, s], up: [0, 0, 1] };
    }
  }
}

/** CAD (Z up) -> three.js (Y up): the part group is rotated -90° about X. */
export const cadToThree = ([x, y, z]: Vec3): Vec3 => [x, z, -y];

/** "60 × 30 × 8 mm", "Ø8 pasante", "Ø20 × 30 (revolución)": the feature's measures, for the legend. */
export function featureMeasures(f: SolidFeature, units: string): string {
  const p = f.profile;
  const fmt = (n: number) => String(Math.round(n * 100) / 100);
  const plan = p.kind === "rect" ? `${fmt(p.w ?? 0)} × ${fmt(p.h ?? 0)}` : p.kind === "circle" ? `Ø${fmt(p.d ?? 0)}` : `polígono de ${p.points.length} lados`;
  if (f.op === "revolve") return `${plan} ${units}, girado 360°`;
  if (f.depth != null) return `${plan} × ${fmt(f.depth)} ${units}`;
  return f.op === "extrude" ? `${plan} ${units}` : `${plan} ${units}, pasante`;
}
