// Asserts the `surface_3d` engine and the `function_plot` area: the math the
// frontend draws (geodesics, descents, level curves, volumes, integrals,
// Riemann sums), the gallery presets, and the static render.
// Run with: bun scripts/check-surface.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { resolveSurfaceTarget } from "../src/features/notebook/blocks/scene3d/Surface3dVisual";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { integrate, riemannSum } from "../src/lib/calculus";
import { compileSurface, descent, displayScale, geodesic, levelCurve, sampleSurface, surfaceIssues, volumeUnder } from "../src/lib/scene3d/surfaceBuild";
import { DynamicSectionBlockSchema, StaticVisualSpecSchema } from "../src/lib/schemas";
import { scoreVisual } from "../src/lib/visualQuality";

const b = { xr: [-4, 4] as [number, number], yr: [-4, 4] as [number, number] };

// --- geodesics are computed, not drawn -------------------------------------------
// On a plane, a geodesic is a straight line.
const plane = compileSurface("0.3*x + 0.2*y", {})!;
const line = geodesic(plane, b, [-4, 1], [1, 0]);
assert.ok(line.length > 100);
assert.ok(line.every((p) => Math.abs(p[1] - 1) < 1e-9), "straight on a plane");
// In a gravity well, two parallel geodesics bend towards the mass (symmetric),
// and the one far away bends less: lensing, with no force anywhere.
const well = compileSurface("-m / sqrt(x^2 + y^2 + 0.5)", { m: 1.5 })!;
const g = sampleSurface(well, b.xr, b.yr, 33);
const s = displayScale(b.xr, b.yr, g.zMin, g.zMax);
const h = (x: number, y: number) => s * well(x, y);
const end = (y0: number) => {
  const p = geodesic(h, b, [-4, y0], [1, 0]);
  return p[p.length - 1][1];
};
assert.ok(end(1.5) < 0 && end(-1.5) > 0, "near paths bend across the centre");
assert.ok(Math.abs(end(1.5) + end(-1.5)) < 1e-6, "symmetric");
assert.ok(3.2 - end(3.2) < 1.5 - end(1.5), "the far path bends less");
// A heavier mass bends more (the slider must matter).
const heavy = compileSurface("-m / sqrt(x^2 + y^2 + 0.5)", { m: 3 })!;
const heavyEnd = geodesic((x, y) => s * heavy(x, y), b, [-4, 1.5], [1, 0]);
assert.ok(heavyEnd[heavyEnd.length - 1][1] < end(1.5), "more mass, more bending");
// A ball rolls to the bottom; a level curve of a radial well is a circle.
const ball = descent(h, b, [3, 2]);
assert.ok(Math.hypot(ball[ball.length - 1][0], ball[ball.length - 1][1]) < 0.1, "rolls to the bottom");
assert.ok(ball.length < 1500, "and stops there");
const ring = levelCurve(h, b, [2, 0]).map((p) => Math.hypot(p[0], p[1]));
assert.ok(Math.max(...ring) - Math.min(...ring) < 0.01, "a contour of a radial well is a circle");

// --- integrals ----------------------------------------------------------------------
assert.ok(Math.abs(volumeUnder(compileSurface("x + y", {})!, [0, 1, 0, 1]) - 1) < 1e-9);
const sq = (x: number) => x * x;
assert.ok(Math.abs(integrate(sq, 0, 2) - 8 / 3) < 1e-9);
assert.ok(Math.abs(integrate(sq, 2, 0) + 8 / 3) < 1e-9, "reversed limits flip the sign");
const six = riemannSum(sq, 0, 2, 6).total;
const forty = riemannSum(sq, 0, 2, 40).total;
assert.ok(Math.abs(forty - 8 / 3) < Math.abs(six - 8 / 3), "more rectangles, closer to the area");

// --- validation mirrors Rust -----------------------------------------------------------
const base = { expr: "-1 / sqrt(x^2 + y^2)", xRange: b.xr, yRange: b.yr, params: [], paths: [], markers: [] };
assert.ok(surfaceIssues(base).some((i) => i.includes("finita")), "1/r at the origin is caught");
assert.ok(surfaceIssues({ ...base, expr: "2" }).some((i) => i.includes("plana")));
assert.ok(surfaceIssues({ ...base, expr: "x^2", paths: [{ label: "fuera", start: [9, 0] }] }).some((i) => i.includes("empieza fuera")));

// --- the presets are valid blocks and render -----------------------------------------------
const preset = (id: string) => DynamicSectionBlockSchema.parse(BLOCK_CATEGORIES.flatMap((c) => c.presets).find((p) => p.id === id)!.block.content_json);
const gr = preset("amt_gr_geodesics");
if (gr.blockType !== "anchored_micro_theory" || gr.visualAid?.renderEngine !== "surface_3d") throw new Error("the relativity theory draws a surface");
assert.equal(gr.visualIntent, "curved_surface");
assert.deepEqual(scoreVisual(gr.visualAid).issues, []);
assert.equal(resolveSurfaceTarget(gr.visualAid, "Geodésica a +1,5"), "arriba");
assert.equal(resolveSurfaceTarget(gr.visualAid, "masa"), "masa");
let html = renderToStaticMarkup(<StaticVisual visual={gr.visualAid} />);
for (const t of ["Masa", "Geodésica lejana", "Ver todas las trayectorias", "Pausar recorrido"]) assert.ok(html.includes(t), t);

const integral = preset("amt_integral_area");
if (integral.blockType !== "anchored_micro_theory" || integral.visualAid?.renderEngine !== "interactive_module") throw new Error("the integral theory draws an area");
assert.deepEqual(scoreVisual(integral.visualAid).issues, []);
html = renderToStaticMarkup(<StaticVisual visual={integral.visualAid} />);
assert.match(html, /Área de 0 a 1\.8/, "the area readout starts at 60 % of the domain");
assert.match(html, /6 rectángulos suman/);
assert.match(html, /su pendiente en/);

// Off-catalogue path kinds fall back instead of breaking the figure.
const loose = StaticVisualSpecSchema.parse({ renderEngine: "surface_3d", expr: "x^2+y^2", xRange: [-1, 1], yRange: [-1, 1], paths: [{ id: "p", label: "P", kind: "spiral", start: [0.5, 0] }] });
if (loose.renderEngine !== "surface_3d") throw new Error("unreachable");
assert.equal(loose.paths[0].kind, "geodesic");

console.log("check-surface: OK");
