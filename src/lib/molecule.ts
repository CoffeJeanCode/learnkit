// Molecule math for the `molecule_3d` engine: the bond graph at each step of
// a mechanism (mirror of the valence rules in `src-tauri/src/domain/molecule.rs`)
// and a deterministic 3D layout — springs along bonds, repulsion between atoms
// — so the model never writes a coordinate.

import { mulberry32 } from "./rng";

export interface Atom {
  id: string;
  element: string;
  charge?: number;
  label?: string | null;
}
export interface Bond {
  a: string;
  b: string;
  order: number;
}
export interface Step {
  title: string;
  form?: Array<[string, string]>;
  break?: Array<[string, string]>;
  charges?: Record<string, number>;
}

/** CPK colours and radii (Å-ish, scaled for a schematic). */
export const ELEMENT_STYLE: Record<string, { color: string; r: number }> = {
  H: { color: "#e5e7eb", r: 0.25 },
  C: { color: "#6b7280", r: 0.38 },
  N: { color: "#60a5fa", r: 0.37 },
  O: { color: "#f87171", r: 0.36 },
  F: { color: "#86efac", r: 0.33 },
  Cl: { color: "#4ade80", r: 0.45 },
  Br: { color: "#b45309", r: 0.5 },
  I: { color: "#a855f7", r: 0.55 },
  S: { color: "#facc15", r: 0.46 },
  P: { color: "#fb923c", r: 0.45 },
  B: { color: "#fca5a5", r: 0.35 },
  Si: { color: "#d6c7a1", r: 0.48 },
  Li: { color: "#c084fc", r: 0.45 },
  Na: { color: "#a78bfa", r: 0.5 },
  K: { color: "#8b5cf6", r: 0.55 },
  Mg: { color: "#34d399", r: 0.48 },
  Ca: { color: "#10b981", r: 0.52 },
};

export function maxValence(element: string, charge = 0): number | null {
  const base: Record<string, number> = { H: 1, F: 1, Cl: 1, Br: 1, I: 1, Li: 1, Na: 1, K: 1, O: 2, S: 2, Mg: 2, Ca: 2, N: 3, P: 3, B: 3, C: 4, Si: 4 };
  const b = base[element];
  if (b === undefined) return null;
  if (["N", "O", "P", "S"].includes(element) && charge > 0) return b + charge;
  if (element === "C" && charge !== 0) return 3;
  if (charge < 0) return Math.max(0, b + charge);
  return b;
}

const k = (a: string, b: string) => (a <= b ? `${a}|${b}` : `${b}|${a}`);

/** The bond list and charges after `upTo` steps. */
export function stateAt(atoms: Atom[], bonds: Bond[], steps: Step[], upTo: number): { bonds: Bond[]; charges: Record<string, number> } {
  const graph = new Map<string, Bond>(bonds.map((b) => [k(b.a, b.b), { ...b }]));
  const charges: Record<string, number> = Object.fromEntries(atoms.map((a) => [a.id, a.charge ?? 0]));
  for (const s of steps.slice(0, upTo)) {
    for (const [a, b] of s.break ?? []) graph.delete(k(a, b));
    for (const [a, b] of s.form ?? []) {
      const cur = graph.get(k(a, b));
      if (cur) cur.order += 1;
      else graph.set(k(a, b), { a, b, order: 1 });
    }
    Object.assign(charges, s.charges ?? {});
  }
  return { bonds: [...graph.values()], charges };
}

/** Atoms whose bonds exceed their valence (empty = chemically drawable). */
export function valenceIssues(atoms: Atom[], bonds: Bond[], charges: Record<string, number>): string[] {
  return atoms.flatMap((a) => {
    const used = bonds.filter((b) => b.a === a.id || b.b === a.id).reduce((s, b) => s + b.order, 0);
    const max = maxValence(a.element, charges[a.id] ?? 0);
    return max !== null && used > max ? [`${a.id} (${a.element}) con ${used} enlaces, admite ${max}`] : [];
  });
}

export type Vec3 = [number, number, number];

/** Force-directed 3D layout; `start` positions (e.g. the previous step's) keep
 *  frames continuous. Deterministic for a given input. */
export function layout(atoms: Atom[], bonds: Bond[], start?: Map<string, Vec3>, iterations = 400): Map<string, Vec3> {
  const rand = mulberry32(atoms.length * 7919 + bonds.length);
  const pos = new Map<string, Vec3>(atoms.map((a, i) => [a.id, start?.get(a.id) ?? [Math.cos(i * 2.4) * (1 + rand()), Math.sin(i * 2.4) * (1 + rand()), (rand() - 0.5) * 1.5]]));
  const len = (b: Bond) => {
    const ra = ELEMENT_STYLE[atoms.find((x) => x.id === b.a)?.element ?? "C"]?.r ?? 0.4;
    const rb = ELEMENT_STYLE[atoms.find((x) => x.id === b.b)?.element ?? "C"]?.r ?? 0.4;
    return (ra + rb) * 2.6 - (b.order - 1) * 0.12;
  };
  // Bonded pairs only feel their spring; repulsion keeps everything else apart.
  const bonded = new Set(bonds.flatMap((b) => [`${b.a}|${b.b}`, `${b.b}|${b.a}`]));
  for (let it = 0; it < iterations; it++) {
    const cool = 1 - it / iterations;
    const force = new Map<string, Vec3>(atoms.map((a) => [a.id, [0, 0, 0]]));
    for (let i = 0; i < atoms.length; i++) {
      for (let j = i + 1; j < atoms.length; j++) {
        if (bonded.has(`${atoms[i].id}|${atoms[j].id}`)) continue;
        const p = pos.get(atoms[i].id)!;
        const q = pos.get(atoms[j].id)!;
        const d: Vec3 = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
        const dist2 = Math.max(0.05, d[0] ** 2 + d[1] ** 2 + d[2] ** 2);
        const f = 0.35 / dist2;
        for (let c = 0; c < 3; c++) {
          force.get(atoms[i].id)![c] += d[c] * f;
          force.get(atoms[j].id)![c] -= d[c] * f;
        }
      }
    }
    for (const b of bonds) {
      const p = pos.get(b.a);
      const q = pos.get(b.b);
      if (!p || !q) continue;
      const d: Vec3 = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
      const dist = Math.max(1e-3, Math.hypot(...d));
      const f = (dist - len(b)) * 1.2;
      for (let c = 0; c < 3; c++) {
        force.get(b.a)![c] += (d[c] / dist) * f;
        force.get(b.b)![c] -= (d[c] / dist) * f;
      }
    }
    for (const a of atoms) {
      const p = pos.get(a.id)!;
      const f = force.get(a.id)!;
      for (let c = 0; c < 3; c++) p[c] += Math.max(-0.3, Math.min(0.3, f[c] * 0.15 * (0.3 + cool)));
    }
  }
  // Centre on the origin.
  const centre: Vec3 = [0, 0, 0];
  for (const p of pos.values()) for (let c = 0; c < 3; c++) centre[c] += p[c] / atoms.length;
  for (const p of pos.values()) for (let c = 0; c < 3; c++) p[c] -= centre[c];
  return pos;
}
