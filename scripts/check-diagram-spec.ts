import { compileDiagramSpecToMermaid, isDiagramSpec, type DiagramSpec } from "../src/lib/diagramSpec";

console.log("=== Testing DiagramSpec AST Compiler ===");

const sampleSpec: DiagramSpec = {
  renderEngine: "diagram_spec",
  kind: "flowchart",
  direction: "TD",
  title: "Caché de consultas",
  nodes: [
    { id: "client", label: "Cliente (Web/App)" },
    { id: "cache", label: "¿Existe en Redis?", shape: "diamond" },
    { id: "db", label: "Base de Datos", shape: "cylinder" },
    { id: "response", label: "Devolver respuesta", shape: "round" },
  ],
  edges: [
    { from: "client", to: "cache", label: "GET /api/item" },
    { from: "cache", to: "response", label: "HIT (Caché)", style: "thick" },
    { from: "cache", to: "db", label: "MISS (Consultar)" },
    { from: "db", to: "response", label: "Actualizar y responder", style: "dotted" },
  ],
};

console.assert(isDiagramSpec(sampleSpec), "Debe identificarse como DiagramSpec");
console.log("✓ isDiagramSpec reconoce el AST correctamente");

const mermaidOutput = compileDiagramSpecToMermaid(sampleSpec);
console.assert(mermaidOutput.startsWith("flowchart TD"), "Debe comenzar con flowchart TD");
console.assert(mermaidOutput.includes('client["Cliente (Web/App)"]'), "Debe escapar y formatear el nodo cliente");
console.assert(mermaidOutput.includes('cache{"¿Existe en Redis?"}'), "Debe renderizar rombo con comillas");
console.assert(mermaidOutput.includes('db[("Base de Datos")]'), "Debe renderizar cilindro con comillas");
console.assert(mermaidOutput.includes('==>|"HIT (Caché)"|'), "Debe renderizar arista thick con label escapado");
console.assert(mermaidOutput.includes('-.->|"Actualizar y responder"|'), "Debe renderizar arista dotted");

console.log("✓ Mermaid compilado deterministamente:\n");
console.log(mermaidOutput);

console.log("\n=== Sequence Diagram Test ===");
const seqSpec: DiagramSpec = {
  kind: "sequence",
  nodes: [
    { id: "user", label: "Estudiante" },
    { id: "llm", label: "Agente IA" },
  ],
  edges: [
    { from: "user", to: "llm", label: "Enviar solución con huecos" },
    { from: "llm", to: "user", label: "Calificación pedagógica" },
  ],
};
const seqOutput = compileDiagramSpecToMermaid(seqSpec);
console.assert(seqOutput.startsWith("sequenceDiagram"), "Debe comenzar con sequenceDiagram");
console.assert(seqOutput.includes("Estudiante->>Agente IA: Enviar solución con huecos"), "Debe renderizar secuencia con labels de nodo");
console.log(seqOutput);

console.log("\n=== DiagramSpec Tests PASSED ===");
