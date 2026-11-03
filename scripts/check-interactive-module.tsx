// Asserts the interactive visual modules: the wire shape the Rust side emits
// parses, every module renders its starting state, a locked module says why,
// and a broken formula degrades to a message instead of throwing.
// Run with: bun scripts/check-interactive-module.tsx
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { fmt } from "../src/features/notebook/blocks/InteractiveModule";
import { StaticVisualSpecSchema } from "../src/lib/schemas";

const render = (raw: unknown, locked = false) => renderToStaticMarkup(<StaticVisual visual={StaticVisualSpecSchema.parse(raw)} locked={locked} />);

const hooke = {
  renderEngine: "interactive_module",
  spec: {
    module: "relation_explorer",
    output: { sym: "F", label: "Fuerza", unit: "N", expr: "k * x" },
    variables: [
      { sym: "k", label: "Rigidez", unit: "N/m", min: 10, max: 200, init: 50 },
      { sym: "x", label: "Estiramiento", unit: "m", min: 0, max: 0.5, init: 0.1 },
    ],
  },
  challenge: { prompt: "Logra una fuerza de 5 N", sym: "F", target: 5, tolerance: 0.2 },
  caption: "Ley de Hooke",
};

// relation_explorer: starting value 50 * 0.1 = 5 N, both sliders, a plot, the caption.
let html = render(hooke);
assert.match(html, /Fuerza/);
assert.match(html, /<output>50 N\/m<\/output>/);
assert.match(html, />5<small> N<\/small>/, "k*x = 5 at the initial values");
assert.equal((html.match(/type="range"/g) ?? []).length, 2);
assert.match(html, /<polyline/);
assert.match(html, /Ley de Hooke/);
assert.match(html, /Logra una fuerza de 5 N/);
assert.match(html, /¡Lo lograste!/, "the initial state already satisfies this target");
assert.doesNotMatch(html, /disabled/);

// Locked (a prediction gate that has not been answered): sliders disabled + a reason; no challenge yet.
html = render(hooke, true);
assert.match(html, /Primero haz tu predicción/);
assert.equal((html.match(/disabled=""/g) ?? []).length, 2);
assert.doesNotMatch(html, /Logra una fuerza/);

// function_plot with a tangent.
html = render({ renderEngine: "interactive_module", spec: { module: "function_plot", expr: "a * x^2", domain: [-3, 3], params: [{ sym: "a", label: "a", min: -2, max: 2, init: 1 }], tangent: true }, caption: "Parábola" });
assert.match(html, /pendiente/);
assert.match(html, /tangente horizontal/, "x0 starts at the vertex (0), where the slope is 0");

// vector_sum, particle_gradient, timeline all render.
html = render({ renderEngine: "interactive_module", spec: { module: "vector_sum", vectors: [{ label: "A", x: 3, y: 0 }, { label: "B", x: 0, y: 4 }], showResultant: true }, caption: "v" });
assert.match(html, /\|R\| = 5/);
html = render({ renderEngine: "interactive_module", spec: { module: "particle_gradient", highLabel: "Fuera", lowLabel: "Dentro", highCount: 20, lowCount: 4, hasBarrier: true, channels: 2 }, caption: "p" });
assert.equal((html.match(/<circle/g) ?? []).length, 24, "one circle per particle");
html = render({ renderEngine: "interactive_module", spec: { module: "timeline", events: [{ label: "Uno", detail: "Primero" }, { label: "Dos", detail: "Segundo" }] }, caption: "t" });
assert.match(html, /Primero/);
assert.doesNotMatch(html, /Segundo/, "only the current event shows its detail");

// A broken formula must not throw.
html = render({ renderEngine: "interactive_module", spec: { module: "relation_explorer", output: { sym: "y", label: "y", expr: "a +" }, variables: [{ sym: "a", label: "a", min: 0, max: 1, init: 0 }] }, caption: "x" });
assert.match(html, /No se pudo calcular/);

// Wire shape: defaults fill the optional booleans, unknown modules are rejected.
const parsed = StaticVisualSpecSchema.parse({ renderEngine: "interactive_module", spec: { module: "vector_sum", vectors: [] }, caption: null });
assert.equal(parsed.renderEngine === "interactive_module" && parsed.spec.module === "vector_sum" && parsed.spec.showResultant, true);
assert.throws(() => StaticVisualSpecSchema.parse({ renderEngine: "interactive_module", spec: { module: "nope" } }));

// diagram_spec node explanations survive the schema.
const dg = StaticVisualSpecSchema.parse({ renderEngine: "diagram_spec", kind: "flowchart", nodes: [{ id: "a", label: "A", explanation: "Qué hace A" }], edges: [] });
assert.equal(dg.renderEngine === "diagram_spec" && dg.nodes[0].explanation, "Qué hace A");

assert.equal(fmt(0.1 + 0.2), "0.3");
assert.equal(fmt(12345.678), "1.23e+4");
assert.equal(fmt(NaN), "—");

console.log("interactive modules: OK");
