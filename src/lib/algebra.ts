// Finite groups for the `algebraic_structure` engine — mirror of
// `src-tauri/src/domain/algebra.rs`. Every group is a set of permutations of
// {0 … n-1}; the model names the group and a subgroup's generators, this
// computes everything drawn (elements, Cayley table, cosets), so Lagrange's
// theorem holds by construction.

export type Perm = number[];
export const GROUP_KINDS = ["cyclic", "dihedral", "symmetric"] as const;
export const MAX_GROUP_ORDER = 24;

const compose = (a: Perm, b: Perm): Perm => b.map((x) => a[x]); // a ∘ b: b first
const key = (p: Perm) => p.join(",");

export function parseElement(kind: string, n: number, name: string): Perm | null {
  const t = name.trim();
  const rot = (k: number): Perm => Array.from({ length: n }, (_, x) => (x + k) % n);
  if (kind === "cyclic") {
    const k = Number(t);
    return Number.isInteger(k) && k >= 0 && k < n && /^\d+$/.test(t) ? rot(k) : null;
  }
  if (kind === "dihedral") {
    if (t === "e") return rot(0);
    const hasS = t.startsWith("s");
    const rest = hasS ? t.slice(1) : t;
    let k: number;
    if (rest === "" && hasS) k = 0;
    else if (rest === "r") k = 1;
    else if (/^r\d+$/.test(rest)) k = Number(rest.slice(1));
    else return null;
    if (k >= n) return null;
    const refl: Perm = Array.from({ length: n }, (_, x) => (n - x) % n);
    return hasS ? compose(refl, rot(k)) : rot(k);
  }
  if (kind === "symmetric") {
    let p: Perm = Array.from({ length: n }, (_, x) => x);
    if (t === "e" || t === "()") return p;
    const cycles: number[][] = [];
    for (const part of t.split(")").map((s) => s.trim()).filter(Boolean)) {
      if (!part.startsWith("(")) return null;
      const pts = part.slice(1).split(/[ ,]+/).filter(Boolean).map(Number);
      if (pts.length < 2 || pts.some((v) => !Number.isInteger(v) || v < 1 || v > n) || new Set(pts).size !== pts.length) return null;
      cycles.push(pts.map((v) => v - 1));
    }
    for (const c of [...cycles].reverse()) {
      const q: Perm = Array.from({ length: n }, (_, x) => x);
      c.forEach((x, j) => (q[x] = c[(j + 1) % c.length]));
      p = compose(q, p);
    }
    return p;
  }
  return null;
}

/** Closure of the generators (BFS order, identity first); null past the limit. */
export function generate(gens: Perm[], n: number, limit = MAX_GROUP_ORDER): Perm[] | null {
  const id = Array.from({ length: n }, (_, x) => x);
  const seen = new Map<string, Perm>([[key(id), id]]);
  const queue = [id];
  while (queue.length) {
    const x = queue.shift()!;
    for (const g of gens) {
      const y = compose(g, x);
      if (!seen.has(key(y))) {
        seen.set(key(y), y);
        if (seen.size > limit) return null;
        queue.push(y);
      }
    }
  }
  return [...seen.values()];
}

export function wholeGroup(kind: string, n: number): Perm[] | null {
  const gens =
    kind === "cyclic"
      ? [parseElement(kind, n, "1")]
      : kind === "dihedral"
        ? [parseElement(kind, n, "r"), parseElement(kind, n, "s")]
        : kind === "symmetric"
          ? [parseElement(kind, n, "(1 2)"), parseElement(kind, n, `(${Array.from({ length: n }, (_, k) => k + 1).join(" ")})`)]
          : [];
  if (!gens.length || gens.some((g) => !g)) return null;
  return generate(gens as Perm[], n);
}

/** A readable name for each element, in the group's own notation. */
export function nameOf(kind: string, n: number, p: Perm): string {
  if (kind === "cyclic") return String(p[0]);
  if (kind === "dihedral") {
    // Rotation r^k sends 0 to k; a reflection s r^k sends 0 to (n - k) % n.
    const k = p[0];
    const isRotation = p[1] === (k + 1) % n;
    if (isRotation) return k === 0 ? "e" : k === 1 ? "r" : `r${k}`;
    const j = (n - k) % n;
    return j === 0 ? "s" : j === 1 ? "sr" : `sr${j}`;
  }
  // Cycle notation, points 1-based.
  const seen = new Set<number>();
  const cycles: string[] = [];
  for (let s = 0; s < p.length; s++) {
    if (seen.has(s) || p[s] === s) continue;
    const c: number[] = [];
    for (let x = s; !seen.has(x); x = p[x]) {
      seen.add(x);
      c.push(x + 1);
    }
    cycles.push(`(${c.join(" ")})`);
  }
  return cycles.join("") || "e";
}

export interface GroupView {
  elements: Perm[];
  names: string[];
  /** table[i][j] = index of elements[i] ∘ elements[j]. */
  table: number[][];
  subgroup: number[];
  /** coset index of every element (left cosets gH), -1 without a subgroup. */
  coset: number[];
  generators: Perm[];
}

export function buildGroup(kind: string, n: number, subgroup: string[]): GroupView | null {
  const elements = wholeGroup(kind, n);
  if (!elements) return null;
  const index = new Map(elements.map((p, i) => [key(p), i]));
  const table = elements.map((a) => elements.map((b) => index.get(key(compose(a, b)))!));
  const gens = subgroup.map((s) => parseElement(kind, n, s)).filter((p): p is Perm => !!p);
  const h = gens.length ? (generate(gens, n) ?? []) : [];
  const hIdx = h.map((p) => index.get(key(p))!).filter((i) => i !== undefined);
  const coset = elements.map(() => -1);
  if (hIdx.length) {
    let c = 0;
    elements.forEach((g, i) => {
      if (coset[i] >= 0) return;
      for (const hi of hIdx) coset[index.get(key(compose(g, elements[hi])))!] = c;
      c++;
    });
  }
  const genOfGroup = kind === "cyclic" ? ["1"] : kind === "dihedral" ? ["r", "s"] : ["(1 2)", `(${Array.from({ length: n }, (_, k) => k + 1).join(" ")})`];
  return {
    elements,
    names: elements.map((p) => nameOf(kind, n, p)),
    table,
    subgroup: hIdx,
    coset,
    generators: genOfGroup.map((g) => parseElement(kind, n, g)!),
  };
}
