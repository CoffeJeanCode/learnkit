// Asserts the pure logic behind the student's knowledge graph: the layout is
// deterministic and keeps every node on screen, relations to missing nodes are
// never drawn, and the "connect what you learned" step only offers EARLIER
// material. Run with: bun scripts/check-concept-graph.ts
import assert from "node:assert/strict";
import type { ClassRecord, Concept, ConceptLink } from "../src/lib/schemas";
import { classLabel, describeLink, earlierNodes, layoutGraph, nodeOptions, truncate, weekLabel, NODE_W } from "../src/features/graph/graph";
import { draftProblem, emptyDraft } from "../src/features/graph/ConnectStep";

const cls = (id: string, order: number, milestone: string, title: string): ClassRecord => ({
  id,
  milestone_id: milestone,
  class_number: order + 1,
  title,
  order_index: order,
  hours: 1,
  objective: null,
  complete: false,
});
const classes = [
  cls("c3", 2, "m2", "Semana 2: Regulación"),
  cls("c1", 0, "m1", "Semana 1: Panorama"),
  cls("c2", 1, "m1", "Semana 1: Reacciones"),
];
const concept = (id: string, classId: string, label: string): Concept => ({ id, courseId: "k", classId, label, createdAtMs: 0 });
const link = (id: string, fromId: string, toId: string, label: string): ConceptLink => ({ id, courseId: "k", fromId, toId, label, createdAtMs: 0 });
const concepts = [concept("k1", "c1", "El ciclo es circular"), concept("k2", "c3", "Se frena a sí mismo")];

// --- labels -------------------------------------------------------------------
assert.equal(weekLabel("Semana 2: Regulación"), "Semana 2");
assert.equal(weekLabel("Sin prefijo"), "");
assert.equal(classLabel("Semana 2: Regulación"), "Regulación");
assert.equal(classLabel("Sin prefijo"), "Sin prefijo");
assert.equal(truncate("corto", 10), "corto");
assert.equal(truncate("una frase bastante larga", 10).length, 10);

// --- layout -------------------------------------------------------------------
const links = [link("l1", "c1", "k2", "explica por qué"), link("l2", "k1", "c2", "viene antes"), link("l3", "c1", "c2", "mismo bloque")];
const layout = layoutGraph(classes, concepts, links);

assert.deepEqual(layout.nodes.map((n) => n.id), ["c1", "k1", "c2", "c3", "k2"], "classes by order, each followed by ITS concepts");
assert.equal(layout.columns.length, 2, "one column per milestone");
assert.deepEqual(layout.columns.map((c) => c.label), ["Semana 1", "Semana 2"]);
for (const n of layout.nodes) {
  assert.ok(n.x >= 0 && n.y >= 0 && n.x + n.w <= layout.width && n.y + n.h <= layout.height, `${n.id} fits the canvas`);
}
const boxes = layout.nodes;
for (let i = 0; i < boxes.length; i++) {
  for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i]!, b = boxes[j]!;
    const overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
    assert.ok(!overlap, `${a.id} and ${b.id} must not overlap`);
  }
}
assert.ok(layout.nodes.find((n) => n.id === "k1")!.x > layout.nodes.find((n) => n.id === "c1")!.x, "concepts are indented under their class");
assert.ok(layout.nodes.find((n) => n.id === "c3")!.x >= NODE_W, "the second week sits in the next column");

assert.deepEqual(layout.edges.map((e) => e.id), ["l1", "l2", "l3"]);
for (const e of layout.edges) {
  assert.match(e.path, /^M [\d.-]+ [\d.-]+ C /, "a cubic curve");
  assert.ok(Number.isFinite(e.labelX) && Number.isFinite(e.labelY), "a placed label");
}
// Same column (l3) bulges to the right of both nodes instead of crossing them,
// and its label is left-anchored OUTSIDE the boxes.
const sameColumn = layout.edges.find((e) => e.id === "l3")!;
assert.equal(sameColumn.labelAnchor, "start");
assert.ok(sameColumn.labelX > layout.nodes.find((n) => n.id === "c1")!.x + NODE_W, "same-column labels sit outside the boxes");
assert.ok(sameColumn.labelX + 24 * 8 <= layout.width, "and still fit inside the canvas");
// A concept and ITS OWN class are the same column even though the concept is indented (l2: k1 -> c2).
assert.equal(layout.edges.find((e) => e.id === "l2")!.labelAnchor, "start", "indentation must not make it look like a cross-column relation");
// Across columns (l1: c1 -> k2) the label is centred in the gap between the boxes.
const cross = layout.edges.find((e) => e.id === "l1")!;
const c1 = layout.nodes.find((n) => n.id === "c1")!;
const k2 = layout.nodes.find((n) => n.id === "k2")!;
assert.equal(cross.labelAnchor, "middle");
assert.ok(cross.labelX > c1.x + c1.w && cross.labelX < k2.x, "the cross-column label sits in the gap, not over a box");
assert.ok(k2.x - (c1.x + c1.w) >= 150, "the gap is wide enough for a label");

// Deterministic: same input, same output.
assert.deepEqual(layoutGraph(classes, concepts, links), layout);

// A relation whose end no longer exists is never drawn.
assert.equal(layoutGraph(classes, concepts, [link("x", "c1", "borrado", "huérfana")]).edges.length, 0);
// No classes: an empty (but valid) canvas.
const empty = layoutGraph([], [], []);
assert.deepEqual([empty.nodes.length, empty.edges.length], [0, 0]);
assert.ok(empty.width > 0 && empty.height > 0);

// --- options for the forms ------------------------------------------------------
const options = nodeOptions(classes, concepts);
assert.deepEqual(options.map((o) => o.id), ["c1", "k1", "c2", "c3", "k2"]);
assert.deepEqual(options.map((o) => o.kind), ["class", "concept", "class", "class", "concept"]);

// "Connect what you learned" offers only EARLIER classes and their concepts.
assert.deepEqual(earlierNodes(classes, concepts, "c1"), [], "the first class has nothing before it");
assert.deepEqual(earlierNodes(classes, concepts, "c2").map((o) => o.id), ["c1", "k1"]);
assert.deepEqual(earlierNodes(classes, concepts, "c3").map((o) => o.id), ["c1", "k1", "c2"], "never the class itself or later ones");
assert.deepEqual(earlierNodes(classes, concepts, "no-existe"), []);

assert.equal(describeLink(links[0]!, options), "Panorama —explica por qué→ Se frena a sí mismo");
assert.equal(describeLink(link("z", "c1", "fantasma", "x"), options), "Panorama —x→ (borrado)");

// --- the connect step never creates a bare, unexplained relation --------------------
assert.equal(draftProblem(emptyDraft()), null, "an empty row is simply ignored");
assert.equal(draftProblem({ idea: "mi idea", targetId: "", relation: "" }), null, "an idea alone is fine");
assert.match(draftProblem({ idea: "mi idea", targetId: "c1", relation: "  " })!, /con tus palabras/);
assert.match(draftProblem({ idea: "mi idea", targetId: "", relation: "porque sí" })!, /Elige con qué/);
assert.equal(draftProblem({ idea: "mi idea", targetId: "c1", relation: "depende de" }), null);

console.log("concept graph rules: OK");
