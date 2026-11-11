// Asserts the `solid_model` engine: the CAD conventions shared with Rust
// (`domain/solid_model.rs`), the real solid built with CSG (volume, every face
// knows its operation), the checks that send a broken part back, walkthrough
// targets (operations, views, the camera) and the static render.
// Run with: bun scripts/check-solid-model.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import * as THREE from "three";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { resolveSolidTarget } from "../src/features/notebook/blocks/scene3d/SolidModelVisual";
import { buildSolid, featureGeometry, signedVolume } from "../src/features/notebook/blocks/scene3d/solidGeometry";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { featureBox, featureMeasures, polygonIsSimple, solidIssues } from "../src/lib/scene3d/solidBuild";
import { DynamicSectionBlockSchema, type SolidFeature, StaticVisualSpecSchema } from "../src/lib/schemas";
import { scoreVisual } from "../src/lib/visualQuality";

const presets = BLOCK_CATEGORIES.flatMap((c) => c.presets);
const preset = (id: string) => {
  const p = presets.find((x) => x.id === id);
  assert.ok(p, id);
  return DynamicSectionBlockSchema.parse(p!.block.content_json);
};

// --- the gallery bracket is a clean part -------------------------------------------
const diagram = preset("s3d_cad_bracket");
if (diagram.blockType !== "declarative_visual_diagram" || diagram.visualAid.renderEngine !== "solid_model") throw new Error("the bracket is a solid_model");
const bracket = diagram.visualAid;
const features: SolidFeature[] = bracket.features;
assert.deepEqual(solidIssues(features), []);
assert.deepEqual(scoreVisual(bracket).issues, []);
assert.equal(diagram.visualIntent, "spatial_shape");

// --- conventions (same as Rust) ------------------------------------------------------
const f = (o: Partial<SolidFeature>): SolidFeature => ({ id: "x", label: "Pieza X", op: "extrude", plane: "xy", profile: { kind: "rect", w: 10, h: 20, points: [] }, at: [0, 10], offset: 0, depth: 5, explanation: "e", ...o });
assert.deepEqual(featureBox(f({ plane: "xz" }), 100), { min: [-5, 0, 0], max: [5, 5, 20] }, "xz: u = x, v = z, normal +Y");
assert.deepEqual(featureBox(f({ plane: "yz" }), 100), { min: [0, -5, 0], max: [5, 5, 20] }, "yz: u = y, v = z, normal +X");
assert.ok(polygonIsSimple([[0, 0], [10, 0], [10, 5], [0, 5]]));
assert.ok(!polygonIsSimple([[0, 0], [10, 5], [10, 0], [0, 5]]), "a bow-tie crosses itself");

// --- the real solid ------------------------------------------------------------------
for (const plane of ["xy", "xz", "yz"] as const) {
  const g = featureGeometry(f({ plane }), 100)!;
  assert.ok(Math.abs(signedVolume(g) - 10 * 20 * 5) < 1e-6, `${plane}: faces point out and the volume is right`);
  const lathe = featureGeometry(f({ plane, op: "revolve", at: [5, 10], depth: null }), 100)!;
  assert.ok(Math.abs(signedVolume(lathe) / (Math.PI * 100 * 20) - 1) < 0.01, `${plane}: revolve is a cylinder of r 10`);
}
const solid = buildSolid(features, features.length, () => new THREE.MeshStandardMaterial())!;
const expected = 60 * 30 * 8 + 8 * 30 * 32 - Math.PI * 16 * 8;
assert.ok(Math.abs(signedVolume(solid.geometry) - expected) / expected < 0.01, `bracket volume ${signedVolume(solid.geometry)} vs ${expected}`);
// Every face group carries the operation that made it: the hole's wall included.
const owners = new Set(solid.geometry.groups.map((gr) => ((solid.material as THREE.Material[])[gr.materialIndex ?? 0].userData.featureId as string)));
assert.deepEqual([...owners].sort(), ["agujero", "base", "pared"]);
// Step by step: after the first operation only the base exists.
assert.ok(Math.abs(signedVolume(buildSolid(features, 1, () => new THREE.MeshStandardMaterial())!.geometry) - 60 * 30 * 8) < 1);

// --- broken parts are sent back ------------------------------------------------------
const issues = (patch: (fs: SolidFeature[]) => void) => {
  const copy = structuredClone(features);
  patch(copy);
  return solidIssues(copy).join(" | ");
};
assert.match(issues((fs) => (fs[1].offset = 60)), /flota separada/);
assert.match(issues((fs) => (fs[2].at = [300, 0])), /no toca la pieza/);
assert.match(issues((fs) => (fs[2].profile.d = 200)), /borraría la pieza|desproporcionadas/);
assert.match(issues((fs) => (fs[1].label = "pieza 2")), /sin etiqueta concreta/);
assert.match(issues((fs) => fs.reverse()), /no crea material/);
assert.match(issues((fs) => (fs[0].depth = null)), /sin profundidad/);

// --- what the student reads -----------------------------------------------------------
assert.equal(featureMeasures(features[0], "mm"), "60 × 30 × 8 mm");
assert.equal(featureMeasures(features[2], "mm"), "Ø8 mm, pasante");
assert.deepEqual(resolveSolidTarget(features, "front"), { view: "front" });
assert.deepEqual(resolveSolidTarget(features, "camera"), { camera: true });
assert.deepEqual(resolveSolidTarget(features, "Agujero Ø8 pasante"), { feature: "agujero" });
assert.equal(resolveSolidTarget(features, "nada"), null);

// Static render (no WebGL here): views, operation stepper and the full list.
let html = renderToStaticMarkup(<StaticVisual visual={bracket} />);
for (const text of ["Isométrica", "Frontal", "Superior", "Derecha", "Operación 3 de 3", "Agujero Ø8 pasante", "Ver todas las operaciones", "Ver tu cámara"]) assert.ok(html.includes(text), text);

// --- the CAD theory draws its example in 3D, with the camera shown ----------------------
const theory = preset("amt_cad_orbit");
if (theory.blockType !== "anchored_micro_theory" || theory.visualAid?.renderEngine !== "solid_model") throw new Error("the orbit theory draws the part");
assert.equal(theory.visualAid.showCamera, true);
assert.deepEqual(scoreVisual(theory.visualAid).issues, []);
html = renderToStaticMarkup(<StaticVisual visual={theory.visualAid} />);
assert.ok(html.includes("Tu cámara, vista desde fuera") && html.includes("Ocultar tu cámara"), "showCamera opens the inset");

// Off-catalogue words never break the figure (Rust repairs them; zod falls back).
const loose = StaticVisualSpecSchema.parse({ renderEngine: "solid_model", startView: "diagonal", features: [{ id: "a", label: "Bloque 10", op: "pad", plane: "XY", profile: { kind: "box", w: 10, h: 10 }, depth: 4, explanation: "e" }] });
if (loose.renderEngine !== "solid_model") throw new Error("unreachable");
assert.equal(loose.startView, "iso");
assert.deepEqual([loose.features[0].op, loose.features[0].plane, loose.features[0].profile.kind], ["extrude", "xy", "rect"]);

console.log("check-solid-model: OK");
