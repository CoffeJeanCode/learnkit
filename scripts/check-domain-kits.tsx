// Asserts the domain kits (phase E) and the code tier (phase F): the math
// each engine computes (groups and cosets, chord schedules, molecule
// valence and layout, illustration layout), the sandbox document of a code
// figure, and that every gallery preset is a clean, renderable block.
// Run with: bun scripts/check-domain-kits.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { codeFigureDocument } from "../src/features/notebook/blocks/codeFigureSdk";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { buildGroup, nameOf, parseElement, wholeGroup } from "../src/lib/algebra";
import { CELL_W, labelBox, slotCentre } from "../src/lib/illustration";
import { chordFrequencies, schedule, shapeOf } from "../src/lib/instrument";
import { layout, stateAt, valenceIssues } from "../src/lib/molecule";
import { DynamicSectionBlockSchema, StaticVisualSpecSchema } from "../src/lib/schemas";
import { scoreVisual } from "../src/lib/visualQuality";

// --- groups: same orders and cosets as the Rust side ------------------------------
assert.equal(wholeGroup("cyclic", 12)!.length, 12);
assert.equal(wholeGroup("dihedral", 4)!.length, 8);
assert.equal(wholeGroup("symmetric", 3)!.length, 6);
assert.equal(wholeGroup("symmetric", 4)!.length, 24);
const z12 = buildGroup("cyclic", 12, ["4"])!;
assert.equal(z12.subgroup.length, 3, "<4> = {0, 4, 8}");
const sizes = new Map<number, number>();
z12.coset.forEach((c) => sizes.set(c, (sizes.get(c) ?? 0) + 1));
assert.equal(sizes.size, 4, "index 4");
assert.ok([...sizes.values()].every((n) => n === 3), "every coset has |H| elements (Lagrange)");
// Every element of D4 is named and parses back to itself.
for (const p of wholeGroup("dihedral", 4)!) {
  const name = nameOf("dihedral", 4, p);
  assert.deepEqual(parseElement("dihedral", 4, name), p, name);
}
assert.equal(nameOf("symmetric", 3, parseElement("symmetric", 3, "(1 2 3)")!), "(1 2 3)");
assert.equal(parseElement("cyclic", 12, "13"), null);

// --- guitar ------------------------------------------------------------------------
assert.deepEqual(shapeOf("G"), [3, 2, 0, 0, 0, 3]);
assert.equal(chordFrequencies("D").length, 4, "D sounds only four strings");
assert.ok(Math.abs(chordFrequencies("Em")[0] - 82.41) < 0.1, "low E open");
const plan = schedule([{ chord: "G", beats: 4 }, { chord: "C", beats: 4 }], "D-DU-UDU", 80);
assert.equal(plan.events.length, 12, "six strums per bar");
assert.ok(Math.abs(plan.total - 6) < 1e-9, "8 beats at 80 bpm = 6 s");
assert.ok(plan.events.every((e, i, a) => i === 0 || e.at > a[i - 1].at), "in time order");

// --- molecules ---------------------------------------------------------------------
const atoms = [
  { id: "C", element: "C", charge: 0 },
  { id: "H1", element: "H", charge: 0 },
  { id: "H2", element: "H", charge: 0 },
  { id: "H3", element: "H", charge: 0 },
  { id: "Br", element: "Br", charge: 0 },
  { id: "O", element: "O", charge: -1 },
  { id: "Ho", element: "H", charge: 0 },
];
const bonds = [
  { a: "C", b: "H1", order: 1 },
  { a: "C", b: "H2", order: 1 },
  { a: "C", b: "H3", order: 1 },
  { a: "C", b: "Br", order: 1 },
  { a: "O", b: "Ho", order: 1 },
];
const good = [{ title: "SN2", form: [["O", "C"]] as Array<[string, string]>, break: [["C", "Br"]] as Array<[string, string]>, charges: { O: 0, Br: -1 } }];
const after = stateAt(atoms, bonds, good, 1);
assert.deepEqual(valenceIssues(atoms, after.bonds, after.charges), []);
const bad = stateAt(atoms, bonds, [{ title: "x", form: [["O", "C"]], charges: { O: 0 } }], 1);
assert.ok(valenceIssues(atoms, bad.bonds, bad.charges).some((s) => s.includes("C (C) con 5")));
const pos = layout(atoms, bonds);
const d = (a: string, b: string) => Math.hypot(...pos.get(a)!.map((v, k) => v - pos.get(b)![k]));
assert.ok(d("C", "H1") > 0.6 && d("C", "H1") < 2.5, `bonded atoms sit at a bond length (${d("C", "H1").toFixed(2)})`);
assert.ok(d("H1", "Ho") > d("C", "H1"), "unbonded atoms are farther apart");

// --- illustration layout --------------------------------------------------------------
const [x0] = slotCentre([0, 0]);
const [x1] = slotCentre([1, 0]);
assert.equal(x1 - x0, CELL_W);
const a = labelBox({ id: "a", kind: "block", label: "Bloque con nombre largo", slot: [1, 1] });
const b = labelBox({ id: "b", kind: "block", label: "Otro bloque largo", slot: [2, 1] });
assert.ok(a.x + a.w <= b.x + 1, "labels of neighbouring slots never overlap");

// --- code figures: the sandbox document ------------------------------------------------
const doc = codeFigureDocument("fig.onFrame(() => fig.clear()); const s = '</script><b>';", 300);
assert.match(doc, /Content-Security-Policy" content="default-src 'none'/);
assert.ok(!doc.includes("const s = '</script>"), "a </script> in the code cannot close the tag");
const sneaky = StaticVisualSpecSchema.parse({ renderEngine: "code_figure", code: "fig.clear(); fetch('https://x')", caption: "c" });
assert.ok(scoreVisual(sneaky).issues.some((i) => i.includes("prohibida")));

// --- gallery presets ----------------------------------------------------------------------
const kits = BLOCK_CATEGORIES.find((c) => c.id === "domain_kits")?.presets ?? [];
assert.equal(kits.length, 5);
for (const p of kits) {
  const block = DynamicSectionBlockSchema.parse(p.block.content_json);
  if (block.blockType !== "declarative_visual_diagram") throw new Error(p.id);
  assert.deepEqual(scoreVisual(block.visualAid).issues, [], p.id);
  const html = renderToStaticMarkup(<StaticVisual visual={block.visualAid} />);
  // A code figure builds its sandbox iframe in an effect: statically it is its frame and caption.
  if (block.visualAid.renderEngine === "code_figure") assert.ok(html.includes(block.visualAid.caption), `${p.id} renders its caption`);
  else assert.ok(html.length > 200, `${p.id} renders`);
}
const lagrange = DynamicSectionBlockSchema.parse(kits.find((p) => p.id === "kit_lagrange")!.block.content_json);
if (lagrange.blockType === "declarative_visual_diagram") {
  const html = renderToStaticMarkup(<StaticVisual visual={lagrange.visualAid} />);
  assert.match(html, /12 = 3 × 4/, "Lagrange is read off the figure");
}
const guitar = DynamicSectionBlockSchema.parse(kits.find((p) => p.id === "kit_guitar")!.block.content_json);
if (guitar.blockType === "declarative_visual_diagram") {
  assert.match(renderToStaticMarkup(<StaticVisual visual={guitar.visualAid} />), /Escuchar/);
}

console.log("check-domain-kits: OK");
