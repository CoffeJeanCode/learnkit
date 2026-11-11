// Numeric integration for the `function_plot` area (the integral as
// accumulated area): what the student reads is computed, never written by
// the model. Pure, so `scripts/check-interactive-module.tsx` can test it.

export type Fn1 = (x: number) => number;

/** ∫ₐᵇ f by composite Simpson (n even). Signed: b < a flips the sign. */
export function integrate(f: Fn1, a: number, b: number, n = 200): number {
  if (a === b) return 0;
  if (b < a) return -integrate(f, b, a, n);
  const m = n % 2 === 0 ? n : n + 1;
  const h = (b - a) / m;
  let s = f(a) + f(b);
  for (let i = 1; i < m; i++) s += (i % 2 ? 4 : 2) * f(a + i * h);
  return (s * h) / 3;
}

/** Midpoint rectangles of a Riemann sum: their [x0, x1, height] and total. */
export function riemannSum(f: Fn1, a: number, b: number, n: number): { rects: Array<[number, number, number]>; total: number } {
  const k = Math.max(1, Math.round(n));
  const w = (b - a) / k;
  const rects: Array<[number, number, number]> = [];
  let total = 0;
  for (let i = 0; i < k; i++) {
    const x0 = a + i * w;
    const y = f(x0 + w / 2);
    rects.push([x0, x0 + w, y]);
    if (Number.isFinite(y)) total += y * w;
  }
  return { rects, total };
}
