// Pure placement for the `explorable_model` engine — no three.js here. The
// model only says WHICH zone a part lives in; this file turns zones into
// positions that do not overlap, deterministically (same spec, same picture).

import { hashString, mulberry32 } from "../rng";

export type Vec3 = [number, number, number];

export interface PartLike {
  id: string;
  shape: string;
  size?: string | null;
  count: number;
  zone?: string | null;
}

export interface ShellLike {
  shape: string;
}

export interface PlacedInstance {
  partId: string;
  /** Index among the copies of the same part. */
  index: number;
  position: Vec3;
  /** Bounding radius of this copy, in scene units. */
  radius: number;
  /** Euler rotation, so copies do not all face the same way. */
  rotation: Vec3;
}

/** Half-extents of each shell shape (the unit the zones are measured in). */
export const SHELL_RADII: Record<string, Vec3> = {
  sphere: [1.5, 1.5, 1.5],
  ellipsoid: [1.95, 1.3, 1.5],
  box: [1.6, 1.25, 1.6],
  cylinder: [1.35, 1.5, 1.35],
  capsule: [2.0, 1.15, 1.15],
};

/** Direction the cutaway opens towards (the default camera sits on this side). */
export const CUT_NORMAL: Vec3 = [0.6, 0.25, 0.76];
/** Shell points with `dot(p, CUT_NORMAL) > CUT_OFFSET` are removed by the cut. */
export const CUT_OFFSET = 0.15;

const SIZE_RADIUS: Record<string, number> = { small: 0.16, medium: 0.24, large: 0.4 };
/** Radial band of each zone, as a fraction of the shell's half-extents. */
const ZONE_BAND: Record<string, [number, number]> = {
  center: [0, 0],
  inner: [0.3, 0.52],
  outer: [0.6, 0.8],
  surface: [1, 1],
  outside: [1.35, 1.6],
};

/** A `layer` part is a concentric shell, not an object placed in a band. */
export function isLayer(part: Pick<PartLike, "shape">): boolean {
  return part.shape === "layer";
}

const ZONE_RANK: Record<string, number> = { center: 0, inner: 1, outer: 2, surface: 3, outside: 4 };
/** More layers than this stop reading as distinct bands. */
export const MAX_LAYERS = 5;

/**
 * Outer surface of each layer as a fraction of the shell's half-extents,
 * inside-out by zone (ties keep the model's order). The layers split the
 * radius evenly, so a 4-layer Earth gets 0.25 / 0.5 / 0.75 / 1; an
 * `outside` layer (an atmosphere) sits beyond the envelope.
 */
export function layerFractions(parts: PartLike[]): Map<string, number> {
  const layers = parts
    .map((p, i) => ({ p, i }))
    .filter(({ p }) => isLayer(p))
    .sort((a, b) => (ZONE_RANK[a.p.zone ?? "inner"] ?? 1) - (ZONE_RANK[b.p.zone ?? "inner"] ?? 1) || a.i - b.i);
  const inside = layers.filter(({ p }) => p.zone !== "outside");
  const out = new Map<string, number>();
  inside.forEach(({ p }, k) => out.set(p.id, (k + 1) / inside.length));
  layers.filter(({ p }) => p.zone === "outside").forEach(({ p }, k) => out.set(p.id, 1.15 + 0.12 * k));
  return out;
}

export function clampCount(count: number): number {
  return Math.min(12, Math.max(1, Math.round(Number.isFinite(count) ? count : 1)));
}

export function instanceRadius(part: PartLike): number {
  const base = SIZE_RADIUS[part.size ?? "medium"] ?? SIZE_RADIUS.medium;
  const n = clampCount(part.count);
  // Many copies shrink so a crowd of ribosomes still fits inside the shell.
  const crowd = n >= 7 ? 0.6 : n >= 3 ? 0.7 : n === 2 ? 0.85 : 1;
  const centre = part.zone === "center" ? 1.2 : 1;
  return base * crowd * centre;
}

const dist = (a: Vec3, b: Vec3) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

export function placeParts(parts: PartLike[], shell?: ShellLike | null, cutaway = false): PlacedInstance[] {
  const radii = SHELL_RADII[shell?.shape ?? "sphere"] ?? SHELL_RADII.sphere;
  const rnd = mulberry32(hashString(parts.map((p) => `${p.id}:${p.shape}:${p.count}:${p.zone}`).join("|")) + 17);
  const placed: PlacedInstance[] = [];
  // Big things first: they are the hardest to fit, small ones fill the gaps.
  // Layers are drawn as nested shells (`layerFractions`), never placed here.
  const order = parts.map((p, i) => ({ p, i })).filter(({ p }) => !isLayer(p)).sort((a, b) => instanceRadius(b.p) - instanceRadius(a.p) || a.i - b.i);

  for (const { p } of order) {
    const zone = p.zone && ZONE_BAND[p.zone] ? p.zone : "inner";
    const [lo, hi] = ZONE_BAND[zone];
    const radius = instanceRadius(p);
    const n = zone === "center" ? 1 : clampCount(p.count);
    for (let index = 0; index < n; index++) {
      let best: Vec3 = [0, 0, 0];
      let bestClearance = -Infinity;
      const tries = zone === "center" ? 1 : 80;
      for (let t = 0; t < tries; t++) {
        // Uniform direction on the sphere.
        const u = rnd() * 2 - 1;
        const phi = rnd() * Math.PI * 2;
        const s = Math.sqrt(1 - u * u);
        const dir: Vec3 = [s * Math.cos(phi), u, s * Math.sin(phi)];
        // Parts sitting ON the shell must stay on the half that is still
        // there when the shell is cut open.
        if (zone === "surface" && cutaway && dir[0] * CUT_NORMAL[0] + dir[1] * CUT_NORMAL[1] + dir[2] * CUT_NORMAL[2] > -0.05) continue;
        const r = lo + rnd() * (hi - lo);
        const candidate: Vec3 = [dir[0] * r * radii[0], dir[1] * r * radii[1], dir[2] * r * radii[2]];
        let clearance = Infinity;
        for (const other of placed) clearance = Math.min(clearance, dist(candidate, other.position) - (radius + other.radius) * 1.08);
        if (clearance > bestClearance) {
          bestClearance = clearance;
          best = candidate;
        }
        if (clearance > 0.04) break;
      }
      placed.push({
        partId: p.id,
        index,
        position: best,
        radius,
        rotation: zone === "center" ? [0, 0, 0] : [rnd() * Math.PI, rnd() * Math.PI, rnd() * Math.PI],
      });
    }
  }
  return placed;
}

/** Total overlap depth between copies of DIFFERENT parts (0 = clean). */
export function overlapDepth(placed: PlacedInstance[]): number {
  let total = 0;
  for (let a = 0; a < placed.length; a++) {
    for (let b = a + 1; b < placed.length; b++) {
      if (placed[a].partId === placed[b].partId) continue;
      total += Math.max(0, placed[a].radius + placed[b].radius - dist(placed[a].position, placed[b].position));
    }
  }
  return total;
}
