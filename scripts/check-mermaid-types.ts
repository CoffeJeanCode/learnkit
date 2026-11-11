// Every Mermaid chart type the generator may emit really parses in Mermaid 12
// with the minimal syntax the prompt teaches (`P_VISUALS` in
// `block_generator_agent.rs`), the header detection agrees with Rust's
// `MermaidChartType::from_code`, and a header that disagrees with
// `chartType` is reported. Run with: bun scripts/check-mermaid-types.ts
import assert from "node:assert/strict";
import { Window } from "happy-dom";

// mermaid -> dompurify needs a real DOM before it is imported.
const win = new Window({ url: "https://localhost/" });
const g = globalThis as Record<string, unknown>;
for (const key of ["window", "document", "navigator", "location", "Node", "Element", "HTMLElement", "SVGElement", "Document", "DocumentFragment", "DOMParser", "NodeFilter", "MutationObserver", "XMLSerializer", "getComputedStyle", "requestAnimationFrame", "cancelAnimationFrame"]) {
  if (g[key] === undefined && (win as unknown as Record<string, unknown>)[key] !== undefined) {
    g[key] = (win as unknown as Record<string, unknown>)[key];
  }
}

const mermaid = (await import("mermaid")).default;
const { mermaidTypeOf } = await import("../src/lib/mermaidTypes");
const { MERMAID_CHART_TYPES, StaticVisualSpecSchema } = await import("../src/lib/schemas");
const { scoreVisual } = await import("../src/lib/visualQuality");
mermaid.initialize({ startOnLoad: false });

/** The prompt's examples, one per type (each `/` there is a newline here). */
const SAMPLES: Record<(typeof MERMAID_CHART_TYPES)[number], string> = {
  flowchart: "flowchart TD\n  A[Boceto] --> B{¿Cerrado?}\n  B -->|sí| C[Extruir]",
  sequenceDiagram: "sequenceDiagram\n  Cliente->>Servidor: GET /\n  Servidor-->>Cliente: 200 OK",
  classDiagram: "classDiagram\n  class Pieza {\n    +material\n  }\n  Pieza <|-- Escuadra",
  stateDiagram: "stateDiagram-v2\n  [*] --> Boceto\n  Boceto --> Solido: extruir",
  erDiagram: "erDiagram\n  PIEZA ||--o{ OPERACION : tiene",
  mindmap: "mindmap\n  root((CAD))\n    Croquis\n      Restricciones\n    Operaciones",
  timeline: "timeline\n  title Historia del CAD\n  1963 : Sketchpad\n  1982 : AutoCAD",
  gantt: "gantt\n  dateFormat YYYY-MM-DD\n  section Diseño\n  Bocetos :a1, 2025-01-06, 5d\n  Modelo :after a1, 4d",
  pie: 'pie title Material usado\n  "PLA" : 60\n  "PETG" : 40',
  quadrantChart: "quadrantChart\n  x-axis Bajo esfuerzo --> Alto esfuerzo\n  y-axis Bajo impacto --> Alto impacto\n  quadrant-1 Hacer ya\n  Tarea A: [0.3, 0.8]",
  journey: "journey\n  section Comprar\n    Buscar: 3: Cliente",
  gitGraph: "gitGraph\n  commit\n  branch feature\n  commit\n  checkout main\n  merge feature",
  xychart: 'xychart-beta\n  x-axis [ene, feb, mar]\n  y-axis "Piezas" 0 --> 100\n  bar [20, 45, 80]',
  sankey: "sankey-beta\n  Solar,Red,40\n  Red,Hogares,30",
};

for (const type of MERMAID_CHART_TYPES) {
  const code = SAMPLES[type];
  assert.equal(mermaidTypeOf(code), type, `header of ${type}`);
  try {
    await mermaid.parse(code);
  } catch (e) {
    assert.fail(`${type} does not parse: ${e instanceof Error ? e.message : String(e)}`);
  }
  const visual = StaticVisualSpecSchema.parse({ renderEngine: "mermaid", chartType: type, code, caption: "c" });
  assert.deepEqual(scoreVisual(visual).issues, [], type);
}

// A chartType that disagrees with the code's header is flagged.
const mismatched = StaticVisualSpecSchema.parse({ renderEngine: "mermaid", chartType: "flowchart", code: SAMPLES.mindmap, caption: "c" });
assert.ok(scoreVisual(mismatched).issues.some((i) => i.includes("abre como mindmap")));

console.log(`check-mermaid-types: OK (${MERMAID_CHART_TYPES.length} chart types parse)`);
