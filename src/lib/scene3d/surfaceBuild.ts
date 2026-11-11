// Pure math of the `surface_3d` engine — no three.js, so the checks can run
// it. The model writes `z = f(x, y)` and WHERE paths start; every curve drawn
// on the surface is computed here, correct by construction:
// - geodesic: the straightest line ON the displayed surface (what a free body
//   follows in an embedding diagram), integrated with RK4 from the geodesic
//   equations of a graph h(x, y):
//     x'' = -h_x · Q / (1 + h_x² + h_y²),  y'' = -h_y · Q / (1 + h_x² + h_y²),
//     Q = h_xx x'² + 2 h_xy x'y' + h_yy y'²
// - descent: where a ball rolls (steepest descent, -∇h);
// - level: the contour through the start (perpendicular to ∇h);
// - volume: the double integral of f over a rectangle (midpoint rule).
// The geodesics run on the DISPLAYED surface (height × `scale`), so what the
// student sees bending is exactly what is computed.

import { tryCompileExpr } from "../safeExpr";

export type Fn2 = (x: number, y: number) => number;
export type Pt3 = [number, number, number];

export interface SurfaceGrid {
  n: number;
  /** z[i][j] at x = xs[i], y = ys[j] (raw f, not scaled). */
  z: number[][];
  xs: number[];
  ys: number[];
  zMin: number;
  zMax: number;
}

/** The surface as a function, or null if the formula does not compile. */
export function compileSurface(expr: string, params: Record<string, number>): Fn2 | null {
  const c = tryCompileExpr(expr);
  if (!c) return null;
  return (x, y) => {
    try {
      const v = c.eval({ ...params, x, y });
      return Number.isFinite(v) ? v : NaN;
    } catch {
      return NaN;
    }
  };
}

export function sampleSurface(f: Fn2, xr: [number, number], yr: [number, number], n = 64): SurfaceGrid {
  const xs = Array.from({ length: n }, (_, i) => xr[0] + ((xr[1] - xr[0]) * i) / (n - 1));
  const ys = Array.from({ length: n }, (_, j) => yr[0] + ((yr[1] - yr[0]) * j) / (n - 1));
  let zMin = Infinity;
  let zMax = -Infinity;
  const z = xs.map((x) =>
    ys.map((y) => {
      const v = f(x, y);
      if (Number.isFinite(v)) {
        zMin = Math.min(zMin, v);
        zMax = Math.max(zMax, v);
      }
      return v;
    }),
  );
  return { n, z, xs, ys, zMin, zMax };
}

/** Vertical exaggeration so the relief reads: its height becomes ~45 % of the
 *  base. Fixed from the INITIAL parameters, so moving a slider deepens the
 *  well on screen instead of being normalised away. */
export function displayScale(xr: [number, number], yr: [number, number], zMin: number, zMax: number): number {
  const span = Math.max(xr[1] - xr[0], yr[1] - yr[0]);
  const relief = zMax - zMin;
  return relief > 1e-12 ? (0.45 * span) / relief : 1;
}

interface Bounds {
  xr: [number, number];
  yr: [number, number];
}

const inside = (b: Bounds, x: number, y: number) => x >= b.xr[0] && x <= b.xr[1] && y >= b.yr[0] && y <= b.yr[1];

/** First and second partial derivatives of h by central differences. */
function derivatives(h: Fn2, x: number, y: number, e: number) {
  const hx = (h(x + e, y) - h(x - e, y)) / (2 * e);
  const hy = (h(x, y + e) - h(x, y - e)) / (2 * e);
  const h0 = h(x, y);
  const hxx = (h(x + e, y) - 2 * h0 + h(x - e, y)) / (e * e);
  const hyy = (h(x, y + e) - 2 * h0 + h(x, y - e)) / (e * e);
  const hxy = (h(x + e, y + e) - h(x + e, y - e) - h(x - e, y + e) + h(x - e, y - e)) / (4 * e * e);
  return { hx, hy, hxx, hyy, hxy };
}

/** Geodesic of the surface z = h(x, y) from `start` heading `dir`. */
export function geodesic(h: Fn2, b: Bounds, start: [number, number], dir: [number, number], maxSteps = 1500): Pt3[] {
  const span = Math.max(b.xr[1] - b.xr[0], b.yr[1] - b.yr[0]);
  const e = span * 1e-3;
  const ds = span / 400;
  const norm = Math.hypot(dir[0], dir[1]) || 1;
  let s = [start[0], start[1], dir[0] / norm, dir[1] / norm];
  const accel = (st: number[]) => {
    const [x, y, vx, vy] = st;
    const d = derivatives(h, x, y, e);
    const q = d.hxx * vx * vx + 2 * d.hxy * vx * vy + d.hyy * vy * vy;
    const den = 1 + d.hx * d.hx + d.hy * d.hy;
    return [vx, vy, (-d.hx * q) / den, (-d.hy * q) / den];
  };
  const out: Pt3[] = [];
  let length = 0;
  for (let i = 0; i < maxSteps && inside(b, s[0], s[1]) && length < span * 3; i++) {
    const z = h(s[0], s[1]);
    if (!Number.isFinite(z)) break;
    out.push([s[0], s[1], z]);
    const k1 = accel(s);
    const k2 = accel(s.map((v, k) => v + (ds / 2) * k1[k]));
    const k3 = accel(s.map((v, k) => v + (ds / 2) * k2[k]));
    const k4 = accel(s.map((v, k) => v + ds * k3[k]));
    const next = s.map((v, k) => v + (ds / 6) * (k1[k] + 2 * k2[k] + 2 * k3[k] + k4[k]));
    if (!next.every(Number.isFinite)) break;
    length += Math.hypot(next[0] - s[0], next[1] - s[1]);
    s = next;
  }
  return out;
}

/** Where a ball released at `start` rolls: steepest descent until it settles. */
export function descent(h: Fn2, b: Bounds, start: [number, number], maxSteps = 1500): Pt3[] {
  const span = Math.max(b.xr[1] - b.xr[0], b.yr[1] - b.yr[0]);
  const e = span * 1e-3;
  const ds = span / 400;
  let [x, y] = start;
  const out: Pt3[] = [];
  for (let i = 0; i < maxSteps && inside(b, x, y); i++) {
    const z = h(x, y);
    if (!Number.isFinite(z)) break;
    out.push([x, y, z]);
    const { hx, hy } = derivatives(h, x, y, e);
    const g = Math.hypot(hx, hy);
    if (g < 1e-6) break; // a minimum (or a flat spot): the ball rests
    const nx = x - (ds * hx) / g;
    const ny = y - (ds * hy) / g;
    if (!(h(nx, ny) < z)) break; // stepped past the bottom: it rests here
    x = nx;
    y = ny;
  }
  return out;
}

/** The contour through `start`: walk perpendicular to the gradient, both ways. */
export function levelCurve(h: Fn2, b: Bounds, start: [number, number], maxSteps = 1200): Pt3[] {
  const span = Math.max(b.xr[1] - b.xr[0], b.yr[1] - b.yr[0]);
  const e = span * 1e-3;
  const ds = span / 400;
  const z0 = h(start[0], start[1]);
  const walk = (sign: 1 | -1): Pt3[] => {
    let [x, y] = start;
    const pts: Pt3[] = [];
    for (let i = 0; i < maxSteps && inside(b, x, y); i++) {
      const { hx, hy } = derivatives(h, x, y, e);
      const g = Math.hypot(hx, hy);
      if (g < 1e-9) break;
      x += (sign * ds * -hy) / g;
      y += (sign * ds * hx) / g;
      // Pull back onto the level (one Newton step), so the curve never drifts.
      const d = derivatives(h, x, y, e);
      const g2 = d.hx * d.hx + d.hy * d.hy || 1;
      const err = h(x, y) - z0;
      x -= (err * d.hx) / g2;
      y -= (err * d.hy) / g2;
      pts.push([x, y, z0]);
      // A closed contour comes back to where it started.
      if (i > 20 && Math.hypot(x - start[0], y - start[1]) < ds) break;
    }
    return pts;
  };
  const forward = walk(1);
  const closed = forward.length > 20 && Math.hypot(forward[forward.length - 1][0] - start[0], forward[forward.length - 1][1] - start[1]) < span / 200;
  if (closed) return [[start[0], start[1], z0], ...forward];
  return [...walk(-1).reverse(), [start[0], start[1], z0], ...forward];
}

/** ∬ f over [x0, x1] × [y0, y1] (midpoint rule; plenty for a readout). */
export function volumeUnder(f: Fn2, [x0, x1, y0, y1]: [number, number, number, number], n = 120): number {
  const dx = (x1 - x0) / n;
  const dy = (y1 - y0) / n;
  let sum = 0;
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      const v = f(x0 + (i + 0.5) * dx, y0 + (j + 0.5) * dy);
      if (Number.isFinite(v)) sum += v;
    }
  }
  return sum * dx * dy;
}

export interface TracedPath {
  id: string;
  /** Points on the DISPLAYED surface: [x, y, scale · f]. */
  points: Pt3[];
}

/** Every path of a spec on the displayed surface. */
export function tracePaths(
  f: Fn2,
  scale: number,
  b: Bounds,
  paths: Array<{ id: string; kind: string; start: [number, number]; direction: [number, number] }>,
): TracedPath[] {
  const h: Fn2 = (x, y) => scale * f(x, y);
  return paths.map((p) => ({
    id: p.id,
    points: p.kind === "descent" ? descent(h, b, p.start) : p.kind === "level" ? levelCurve(h, b, p.start) : geodesic(h, b, p.start, p.direction),
  }));
}

/** Issues `visualQuality` reports (mirror of `domain/surface.rs`). */
export function surfaceIssues(spec: {
  expr: string;
  xRange: [number, number];
  yRange: [number, number];
  params: Array<{ sym: string; min: number; max: number; init: number }>;
  paths: Array<{ label: string; start: [number, number] }>;
  markers: Array<{ label: string; at: [number, number] }>;
}): string[] {
  const issues: string[] = [];
  const [xr, yr] = [spec.xRange, spec.yRange];
  if (!(xr[0] < xr[1] && yr[0] < yr[1])) return ["rangos inválidos"];
  const corners = spec.params.length ? [spec.params.map((p) => p.min), spec.params.map((p) => p.max), spec.params.map((p) => p.init)] : [[]];
  for (const values of corners) {
    const env = Object.fromEntries(spec.params.map((p, k) => [p.sym, values[k]]));
    const f = compileSurface(spec.expr, env);
    if (!f) return ["la fórmula no compila"];
    const g = sampleSurface(f, xr, yr, 21);
    if (g.z.some((row) => row.some((v) => !Number.isFinite(v)))) {
      issues.push("la superficie deja de ser finita en el dominio");
      break;
    }
    if (g.zMax - g.zMin < 1e-9) {
      issues.push("la superficie es plana");
      break;
    }
  }
  const on = (p: [number, number]) => p[0] >= xr[0] && p[0] <= xr[1] && p[1] >= yr[0] && p[1] <= yr[1];
  for (const p of spec.paths) if (!on(p.start)) issues.push(`la trayectoria "${p.label}" empieza fuera`);
  for (const m of spec.markers) if (!on(m.at)) issues.push(`el marcador "${m.label}" está fuera`);
  if (spec.paths.length > 4) issues.push(`${spec.paths.length} trayectorias (máximo 4)`);
  return issues;
}
