// Asserts the `sequence_strip` engine: symbols split right, the difference
// between rows is computed exactly (never trusted to the model), numbered
// placeholders are caught, and the figure renders with the change marked.
// Run with: bun scripts/check-sequence-strip.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { DynamicSectionBlockSchema, StaticVisualSpecSchema } from "../src/lib/schemas";
import { diffRow, isPlaceholderLabel, symbolsOf } from "../src/lib/sequenceStrip";
import { scoreVisual } from "../src/lib/visualQuality";

// Symbols: letters, unless the chunk holds words.
assert.deepEqual(symbolsOf("ATG"), ["A", "T", "G"]);
assert.deepEqual(symbolsOf("GET /index"), ["GET", "/index"]);

// The diff is per position, against the first row.
const original = [
  { text: "ATG", meaning: "Metionina · inicio" },
  { text: "GGA", meaning: "Glicina" },
  { text: "TAA", meaning: "Fin" },
];
const mutated = [
  { text: "ATG", meaning: "Metionina · inicio" },
  { text: "GAA", meaning: "Ácido glutámico" },
  { text: "TAA", meaning: "Fin" },
];
const d = diffRow(original, mutated);
assert.deepEqual([...d[1].changedSymbols], [1], "only the middle letter of GGA -> GAA changed");
assert.equal(d[1].meaningChanged, true);
assert.equal(d[0].changedSymbols.size + d[2].changedSymbols.size, 0, "untouched codons stay unmarked");
// A silent mutation: a letter changes, the meaning does not.
const silent = diffRow(original, [original[0], { text: "GGG", meaning: "glicina" }, original[2]]);
assert.equal(silent[1].changedSymbols.size, 1);
assert.equal(silent[1].meaningChanged, false, "same meaning (case aside) is not a change");

// Placeholders name nothing; real labels pass.
for (const p of ["palabra 1", "¿palabra 2?", "palabra 1 · palabra 2 · palabra 3", "Paso 3", "(elemento 4)"]) assert.ok(isPlaceholderLabel(p), p);
for (const ok of ["Glicina", "Paso 1: abrir el socket", "Metionina · inicio", "192 = red local", "palabra"]) assert.ok(!isPlaceholderLabel(ok), ok);

const strip = StaticVisualSpecSchema.parse({
  renderEngine: "sequence_strip",
  rows: [
    { label: "Original", chunks: original },
    { label: "Con la mutación", chunks: mutated },
  ],
  caption: "Cambia una letra, cambia la pieza.",
});
assert.deepEqual(scoreVisual(strip).issues, []);
const html = renderToStaticMarkup(<StaticVisual visual={strip} />);
assert.equal((html.match(/seq-tile is-changed/g) ?? []).length, 1, "exactly one tile is marked");
assert.match(html, /seq-meaning is-changed">Ácido glutámico/);
assert.match(html, /Con la mutación/);
assert.match(html, /Lo marcado en rojo es lo que cambia/);

const vague = StaticVisualSpecSchema.parse({
  renderEngine: "sequence_strip",
  rows: [
    { label: "A", chunks: [{ text: "ATG", meaning: "palabra 1" }, { text: "GGA", meaning: "palabra 2" }] },
    { label: "B", chunks: [{ text: "ATG", meaning: "palabra 1" }] },
  ],
  caption: "x",
});
const issues = scoreVisual(vague).issues;
assert.ok(issues.some((i) => i.includes("sin significado concreto")));
assert.ok(issues.some((i) => i.includes("no tiene los mismos trozos")));

// Rows are variants, never steps: a strip that changes everywhere is rejected.
const steps = StaticVisualSpecSchema.parse({
  renderEngine: "sequence_strip",
  rows: [
    { label: "Paso 1", chunks: [{ text: "SYN", meaning: "quiero hablar" }, { text: "1000", meaning: "mi contador" }, { text: "—", meaning: "nada que acusar" }] },
    { label: "Paso 2", chunks: [{ text: "SYN-ACK", meaning: "de acuerdo" }, { text: "5000", meaning: "mi contador" }, { text: "1001", meaning: "recibí tu 1000" }] },
  ],
  caption: "x",
});
assert.ok(scoreVisual(steps).issues.some((i) => i.includes("son pasos, no una variante")));

// diagram_spec boxes with placeholder labels are flagged too.
const boxes = StaticVisualSpecSchema.parse({
  renderEngine: "diagram_spec",
  kind: "flowchart",
  nodes: [
    { id: "a", label: "ATG GGA TAA" },
    { id: "b", label: "palabra 1 · palabra 2 · palabra 3" },
  ],
  edges: [{ from: "a", to: "b" }],
});
assert.ok(scoreVisual(boxes).issues.some((i) => i.includes("etiqueta genérica")));

// The gallery's DNA theory uses it and is a valid, clean block.
const preset = BLOCK_CATEGORIES.flatMap((c) => c.presets).find((p) => p.id === "amt_dna_codons");
assert.ok(preset, "DNA theory preset exists");
const block = DynamicSectionBlockSchema.parse(preset!.block.content_json);
assert.equal(block.blockType, "anchored_micro_theory");
if (block.blockType !== "anchored_micro_theory" || !block.visualAid) throw new Error("the DNA theory draws its example");
assert.equal(block.visualAid.renderEngine, "sequence_strip");
assert.deepEqual(scoreVisual(block.visualAid).issues, []);

console.log("check-sequence-strip: OK");
