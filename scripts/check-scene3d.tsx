// Asserts the 3D visual engines (topology_flow / explorable_model) WITHOUT a
// GPU: the wire shape the Rust side emits parses, the layout and routing are
// right, part placement is deterministic and overlap-free, and both visuals
// render their full content as plain HTML (which is also what a machine
// without WebGL shows). three.js itself is never loaded here.
// Run with: bun scripts/check-scene3d.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { CAMERA_TARGET, DynamicSectionBlockSchema, SOLID_VIEWS, StaticVisualSpecSchema } from "../src/lib/schemas";
import { layerFractions, overlapDepth, placeParts } from "../src/lib/scene3d/partPlacement";
import { drawableRoute, layoutTopology, placeLabels, routeMessage, topologyToSequence } from "../src/lib/scene3d/topologyLayout";
import { scoreVisual } from "../src/lib/visualQuality";

const render = (raw: unknown, highlight?: string) => renderToStaticMarkup(<StaticVisual visual={StaticVisualSpecSchema.parse(raw)} highlightGroupId={highlight} />);

// --- topology_flow -------------------------------------------------------------

const http = {
  renderEngine: "topology_flow",
  title: "Petición HTTP",
  nodes: [
    { id: "cliente", label: "Cliente", icon: "laptop", subtext: "192.168.1.10", explanation: "Tu navegador." },
    { id: "router", label: "Router", icon: "router" },
    { id: "internet", label: "Internet", icon: "cloud" },
    { id: "dns", label: "Resolutor DNS", icon: "database", subtext: "8.8.8.8" },
    { id: "web", label: "Servidor web", icon: "server", subtext: "203.0.113.10" },
  ],
  links: [
    { from: "cliente", to: "router" },
    { from: "router", to: "internet" },
    { from: "internet", to: "dns" },
    { from: "internet", to: "web" },
  ],
  steps: [
    {
      title: "Resolución DNS",
      detail: "Hay que averiguar la IP.",
      messages: [
        { from: "cliente", to: "dns", label: "Query A www.example.com", tone: "request" },
        { from: "dns", to: "cliente", label: "A 203.0.113.10", tone: "response" },
      ],
    },
    { title: "Intercambio HTTP", detail: "Pide la página.", messages: [{ from: "cliente", to: "web", label: "GET / HTTP/1.1" }] },
  ],
  caption: "Tres conversaciones sobre los mismos cables.",
};

const flow = StaticVisualSpecSchema.parse(http);
assert.equal(flow.renderEngine, "topology_flow");
if (flow.renderEngine !== "topology_flow") throw new Error("unreachable");

// Routing: the model names only the ends; the packet goes through every hop.
assert.deepEqual(routeMessage(flow.nodes, flow.links, "cliente", "dns"), ["cliente", "router", "internet", "dns"]);
assert.deepEqual(routeMessage(flow.nodes, flow.links, "web", "cliente"), ["web", "internet", "router", "cliente"]);
assert.equal(routeMessage(flow.nodes, [], "cliente", "web"), null, "no links, no path");
assert.deepEqual(drawableRoute(flow.nodes, [], "cliente", "web"), ["cliente", "web"], "an unroutable message still draws as a straight hop");
assert.equal(routeMessage(flow.nodes, flow.links, "cliente", "nube"), null, "unknown node");

// Layout: columns follow link distance from the first node, nobody shares a spot.
const layout = layoutTopology(flow.nodes, flow.links);
const x = (id: string) => layout.positions[id].x;
assert.ok(x("cliente") < x("router") && x("router") < x("internet") && x("internet") < x("dns"), "the story reads along one axis");
assert.equal(x("dns"), x("web"), "siblings share a column");
const spots = new Set(Object.values(layout.positions).map((p) => `${p.x},${p.z}`));
assert.equal(spots.size, flow.nodes.length, "no two nodes on the same spot");
// A disconnected island gets its own column instead of landing on another node.
const island = layoutTopology([...flow.nodes, { id: "solo", label: "Solo" }], flow.links);
assert.ok(island.positions.solo.x > island.positions.dns.x);
// Deterministic.
assert.deepEqual(layoutTopology(flow.nodes, flow.links), layout);

// Labels: every node gets a side, and the frame contains them all.
const { sides, bounds } = placeLabels(flow.nodes, layout);
assert.equal(Object.keys(sides).length, flow.nodes.length);
assert.ok(bounds.maxR > bounds.minR && bounds.maxU > bounds.minU);
assert.notEqual(sides.dns, "below", "the label under the DNS would land on the Internet cloud, so it moves");

// The sequence view: participants in order, one note per step, an arrow per message.
const sequence = topologyToSequence(flow.nodes, flow.steps);
assert.match(sequence, /^sequenceDiagram/);
assert.match(sequence, /participant p0 as Cliente/);
assert.match(sequence, /p0->>p3: Query A www\.example\.com/);
assert.match(sequence, /p3-->>p0: A 203\.0\.113\.10/, "a response is a dashed arrow");
assert.match(sequence, /Note over p0,p4: 2\. Intercambio HTTP/);
assert.doesNotMatch(topologyToSequence(flow.nodes, flow.steps, 1), /GET/, "only the steps already played");
assert.doesNotMatch(topologyToSequence([{ id: "a", label: 'Dice "hola"; #1' }], []), /["#;]/, "labels cannot break the Mermaid grammar");

// Render: the full story is in the HTML, playable controls included.
let html = render(http);
assert.match(html, /role="tablist"/);
assert.match(html, />Red<\/button>/);
assert.match(html, />Secuencia<\/button>/);
assert.match(html, /Paso 1 de 2/);
assert.match(html, /Resolución DNS/);
assert.match(html, /Query A www\.example\.com/);
assert.match(html, /<td>Cliente<\/td><td>Resolutor DNS<\/td>/, "the table shows labels, not ids");
assert.match(html, /Reproducir/);
assert.match(html, /Tres conversaciones/);
assert.doesNotMatch(html, /GET \/ HTTP/, "step 2 is not shown before it is reached");

// Quality scoring catches what the backend would reject.
assert.deepEqual(scoreVisual(flow).issues, []);
const unreachable = StaticVisualSpecSchema.parse({ ...http, links: http.links.slice(0, 2) });
assert.ok(scoreVisual(unreachable).issues.some((i) => i.includes("sin camino de links")));

// Tolerance: an off-catalogue icon or tone never breaks a stored block.
const odd = StaticVisualSpecSchema.parse({
  renderEngine: "topology_flow",
  nodes: [
    { id: "a", label: "A", icon: "mainframe", subtext: null },
    { id: "b", label: "B", icon: "server" },
  ],
  links: [{ from: "a", to: "b" }],
  steps: [{ title: "t", messages: [{ from: "a", to: "b", label: "x", tone: "shout" }] }],
});
if (odd.renderEngine !== "topology_flow") throw new Error("unreachable");
assert.equal(odd.nodes[0].icon, "box");
assert.equal(odd.steps[0].messages[0].tone, "request");
assert.equal(odd.steps[0].detail, "");

// --- explorable_model ------------------------------------------------------------

const cell = {
  renderEngine: "explorable_model",
  title: "Célula animal",
  shell: { label: "Membrana plasmática", shape: "sphere" },
  parts: [
    { id: "nucleo", label: "Núcleo", shape: "sphere", size: "large", zone: "center", color: "violet", explanation: "Guarda el ADN." },
    { id: "mitocondria", label: "Mitocondrias", shape: "capsule", size: "medium", count: 4, zone: "outer", color: "rose", explanation: "Producen ATP." },
    { id: "golgi", label: "Aparato de Golgi", shape: "disc_stack", size: "medium", zone: "outer", explanation: "Empaqueta proteínas." },
    { id: "ribosomas", label: "Ribosomas", shape: "grains", size: "small", count: 8, zone: "inner", explanation: "Fabrican proteínas." },
    { id: "receptor", label: "Receptores", shape: "cylinder", size: "small", count: 3, zone: "surface", explanation: "Reciben señales." },
  ],
  caption: "Cada orgánulo tiene un trabajo.",
};

const model = StaticVisualSpecSchema.parse(cell);
if (model.renderEngine !== "explorable_model") throw new Error("unreachable");
assert.equal(model.shell?.cutaway, true, "the shell opens by default");
assert.equal(model.parts[0].count, 1, "count defaults to 1");

const placed = placeParts(model.parts, model.shell, true);
assert.equal(placed.length, 1 + 4 + 1 + 8 + 3, "one instance per copy");
assert.deepEqual(placed.find((p) => p.partId === "nucleo")!.position, [0, 0, 0], "the centre part sits at the centre");
assert.ok(overlapDepth(placed) < 0.05, `parts do not interpenetrate (depth ${overlapDepth(placed).toFixed(3)})`);
assert.deepEqual(placeParts(model.parts, model.shell, true), placed, "same spec, same picture");
const radial = (p: (typeof placed)[number]) => Math.hypot(...p.position);
const inner = placed.filter((p) => p.partId === "ribosomas").map(radial);
const outer = placed.filter((p) => p.partId === "mitocondria").map(radial);
assert.ok(Math.max(...inner) < Math.min(...outer), "inner parts are nearer the centre than outer ones");
// Surface parts stay on the half of the shell that survives the cut.
for (const p of placed.filter((i) => i.partId === "receptor")) {
  assert.ok(p.position[0] * 0.6 + p.position[1] * 0.25 + p.position[2] * 0.76 < 0.15, "a surface part is not floating where the shell was cut away");
}
// An absurd count is clamped, never a thousand meshes.
assert.equal(placeParts([{ id: "x", shape: "sphere", count: 5000, zone: "inner" }]).length, 12);

html = render(cell);
assert.match(html, /Célula animal/);
assert.match(html, /Mitocondrias<span class="scene3d-count">×4<\/span>/);
assert.match(html, /Membrana plasmática/);
assert.match(html, /Ver todas las partes/);
assert.match(html, /<strong>Aparato de Golgi:<\/strong> Empaqueta proteínas\./, "every explanation is reachable without WebGL");
assert.match(html, /Aislar parte/);
assert.match(html, /Cerrar corte/, "a cutaway shell starts cut open");
assert.match(html, /Cada orgánulo tiene un trabajo\./);

assert.deepEqual(scoreVisual(model).issues, []);
const twoCentres = StaticVisualSpecSchema.parse({ ...cell, parts: [...cell.parts, { id: "otro", label: "Otro", shape: "sphere", zone: "center", explanation: "" }] });
const issues = scoreVisual(twoCentres).issues;
assert.ok(issues.some((i) => i.includes("más de una parte en el centro")));
assert.ok(issues.some((i) => i.includes("sin explicación")));

// Layers: concentric shells ordered inside-out by zone, never placed in a band.
const earth = StaticVisualSpecSchema.parse({
  renderEngine: "explorable_model",
  parts: [
    { id: "corteza", label: "Corteza", shape: "layer", zone: "surface", explanation: "Fina y rígida." },
    { id: "nucleo", label: "Núcleo", shape: "layer", zone: "center", explanation: "Metal." },
    { id: "manto", label: "Manto", shape: "layer", zone: "outer", explanation: "Roca que fluye." },
    { id: "placas", label: "Volcanes", shape: "box", size: "small", count: 3, zone: "surface", explanation: "Salidas de magma." },
  ],
  caption: "Capas.",
});
if (earth.renderEngine !== "explorable_model") throw new Error("unreachable");
assert.deepEqual([...layerFractions(earth.parts).entries()], [["nucleo", 1 / 3], ["manto", 2 / 3], ["corteza", 1]], "inside-out by zone, evenly spaced");
assert.deepEqual(new Set(placeParts(earth.parts).map((p) => p.partId)), new Set(["placas"]), "layers are not band-placed instances");
assert.deepEqual(scoreVisual(earth).issues, []);
html = render(earth);
assert.match(html, /Cerrar corte/, "a layered model starts cut open, even without a shell");
assert.doesNotMatch(html, /Corteza<span class="scene3d-count">/, "a layer never shows a copy count");
const sixLayers = StaticVisualSpecSchema.parse({ ...earth, parts: Array.from({ length: 6 }, (_, i) => ({ id: `c${i}`, label: `C${i}`, shape: "layer", zone: "inner", explanation: "x" })) });
assert.ok(scoreVisual(sixLayers).issues.some((i) => i.includes("6 capas")));
// The envelope is a walkthrough target too (`shell`), and its chip is selectable.
assert.match(render(cell, "shell"), /<button[^>]*scene3d-chip[^>]*>.*Membrana plasmática/s);

// --- the gallery presets are real, valid blocks -------------------------------------

const presets = BLOCK_CATEGORIES.find((c) => c.id === "scene_3d")?.presets ?? [];
assert.equal(presets.length, 4);
for (const preset of presets) {
  const block = DynamicSectionBlockSchema.parse(preset.block.content_json);
  assert.equal(block.blockType, "declarative_visual_diagram");
  if (block.blockType !== "declarative_visual_diagram") throw new Error("unreachable");
  assert.deepEqual(scoreVisual(block.visualAid).issues, [], preset.id);
  // Every walkthrough step points at something that exists in the scene.
  const v = block.visualAid;
  const ids =
    v.renderEngine === "topology_flow"
      ? v.nodes.map((n) => n.id)
      : v.renderEngine === "explorable_model"
        ? v.parts.map((p) => p.id)
        : v.renderEngine === "solid_model"
          ? [...v.features.map((f) => f.id), ...SOLID_VIEWS, CAMERA_TARGET]
          : [];
  for (const step of block.guidedWalkthrough) assert.ok(ids.includes(step.targetVisualElement), `${preset.id}: ${step.targetVisualElement}`);
}

console.log("check-scene3d: OK");
