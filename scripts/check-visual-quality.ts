// Asserts the visual-quality metrics catch the failure classes the generator is
// told to avoid, and stay silent on a clean visual.
// Run with: bun scripts/check-visual-quality.ts
import assert from "node:assert/strict";
import { scoreVisual, textBox } from "../src/lib/visualQuality";
import { StaticVisualSpecSchema } from "../src/lib/schemas";

const parse = (raw: unknown) => StaticVisualSpecSchema.parse(raw);

// A clean svg: every text sits on its pill, labels are far apart, palette colours.
const clean = scoreVisual(
  parse({
    renderEngine: "declarative_svg",
    caption: "c",
    groups: [
      {
        groupId: "g",
        pedagogicalRole: "r",
        elements: [
          { tag: "rect", props: { x: 10, y: 10, width: 100, height: 24, fill: "#1e293b", fillOpacity: 0.85 } },
          { tag: "text", props: { x: 16, y: 28, fill: "#cbd5e1", fontSize: 14 }, label: "Membrana" },
          { tag: "rect", props: { x: 10, y: 200, width: 100, height: 24, fill: "#1e293b", fillOpacity: 0.85 } },
          { tag: "text", props: { x: 16, y: 218, fill: "#38bdf8", fontSize: 14 }, label: "Citoplasma" },
        ],
      },
    ],
  }),
);
assert.deepEqual(clean.issues, []);
assert.equal(clean.score, 1);

// The classic failures: overlapping labels, text with no pill, kebab-case, a vanishing colour.
const bad = scoreVisual(
  parse({
    renderEngine: "declarative_svg",
    caption: "c",
    elements: [
      { tag: "text", props: { x: 100, y: 100, fill: "#cbd5e1", fontSize: 14 }, label: "Fuerza neta" },
      { tag: "text", props: { x: 104, y: 104, "fill-opacity": 0.5, fill: "#0f172a", fontSize: 14 }, label: "Fuerza A" },
    ],
  }),
);
assert.equal(bad.stats.overlaps, 1, "two labels on top of each other");
assert.equal(bad.stats.unbackedTexts, 2);
assert.equal(bad.stats.kebabProps, 1);
assert.equal(bad.stats.offPalette, 1);
assert.ok(bad.score < 0.5);

// textBox respects text-anchor.
const mid = textBox({ x: 100, y: 50, textAnchor: "middle", fontSize: 10 }, "abcd");
assert.ok(mid.x0 < 100 && mid.x1 > 100);
const end = textBox({ x: 100, y: 50, textAnchor: "end", fontSize: 10 }, "abcd");
assert.ok(end.x1 <= 100.001);

// diagram_spec: dangling edge + bad id are findings; a sound one is clean.
const dg = scoreVisual(parse({ renderEngine: "diagram_spec", kind: "flowchart", nodes: [{ id: "a b", label: "A" }, { id: "c", label: "C" }], edges: [{ from: "c", to: "zz" }] }));
assert.equal(dg.issues.length, 2);
assert.equal(scoreVisual(parse({ renderEngine: "diagram_spec", kind: "flowchart", nodes: [{ id: "a", label: "A" }, { id: "b", label: "B" }], edges: [{ from: "a", to: "b" }] })).score, 1);

// matrices with ragged rows are flagged.
assert.equal(scoreVisual(parse({ renderEngine: "conceptual_matrix", headers: ["a", "b"], rows: [["1"]], contrastFocus: "x" })).issues.length, 1);

console.log("visual quality: OK");
