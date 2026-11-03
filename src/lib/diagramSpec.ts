export type DiagramKind = "flowchart" | "sequence" | "state" | "cycle" | "compare";

export interface DiagramNode {
  id: string;
  label: string;
  shape?: "rectangle" | "round" | "diamond" | "cylinder" | "circle";
  subtext?: string;
}

export interface DiagramEdge {
  from: string;
  to: string;
  label?: string;
  style?: "solid" | "dotted" | "thick";
}

export interface DiagramSpec {
  renderEngine?: "diagram_spec";
  kind: DiagramKind;
  direction?: "TD" | "LR" | "BT" | "RL";
  title?: string;
  nodes: DiagramNode[];
  edges: DiagramEdge[];
}

/** Sanitiza una etiqueta para evitar romper la gramática de Mermaid */
function sanitizeMermaidLabel(raw: string): string {
  // Elimina saltos de línea peligrosos y escapa comillas dobles
  return raw
    .replace(/"/g, "'")
    .replace(/\n+/g, " ")
    .trim();
}

/** Renderiza la forma del nodo según la sintaxis de Mermaid */
function renderNodeShape(node: DiagramNode): string {
  const label = sanitizeMermaidLabel(node.subtext ? `${node.label} (${node.subtext})` : node.label);
  switch (node.shape) {
    case "diamond":
      return `${node.id}{"${label}"}`;
    case "round":
      return `${node.id}("${label}")`;
    case "cylinder":
      return `${node.id}[("${label}")]`;
    case "circle":
      return `${node.id}(("${label}"))`;
    case "rectangle":
    default:
      return `${node.id}["${label}"]`;
  }
}

/** Renderiza el conector de arista */
function renderEdgeConnector(edge: DiagramEdge): string {
  const edgeLabel = edge.label ? `|"${sanitizeMermaidLabel(edge.label)}"|` : "";
  switch (edge.style) {
    case "dotted":
      return `-.->${edgeLabel}`;
    case "thick":
      return `==>${edgeLabel}`;
    case "solid":
    default:
      return `-->${edgeLabel}`;
  }
}

/**
 * Compila un DiagramSpec AST a código Mermaid 100% determinista y sanitizado.
 * Ahorra ~70% de tokens de salida respecto a pedirle al LLM sintaxis cruda.
 */
export function compileDiagramSpecToMermaid(spec: DiagramSpec): string {
  const direction = spec.direction ?? "TD";

  if (spec.kind === "sequence") {
    const lines = ["sequenceDiagram"];
    for (const edge of spec.edges) {
      const fromNode = spec.nodes.find((n) => n.id === edge.from)?.label ?? edge.from;
      const toNode = spec.nodes.find((n) => n.id === edge.to)?.label ?? edge.to;
      const msg = edge.label ? sanitizeMermaidLabel(edge.label) : "petición";
      lines.push(`  ${fromNode}->>${toNode}: ${msg}`);
    }
    return lines.join("\n");
  }

  if (spec.kind === "state") {
    const lines = ["stateDiagram-v2"];
    for (const node of spec.nodes) {
      lines.push(`  ${node.id}: ${sanitizeMermaidLabel(node.label)}`);
    }
    for (const edge of spec.edges) {
      const label = edge.label ? `: ${sanitizeMermaidLabel(edge.label)}` : "";
      lines.push(`  ${edge.from} --> ${edge.to}${label}`);
    }
    return lines.join("\n");
  }

  // Por defecto: flowchart TD / LR
  const lines = [`flowchart ${direction}`];

  for (const node of spec.nodes) {
    lines.push(`  ${renderNodeShape(node)}`);
  }

  for (const edge of spec.edges) {
    const conn = renderEdgeConnector(edge);
    lines.push(`  ${edge.from} ${conn} ${edge.to}`);
  }

  return lines.join("\n");
}

/**
 * Verifica si un objeto de visual aid es un DiagramSpec AST
 */
export function isDiagramSpec(val: unknown): val is DiagramSpec {
  if (!val || typeof val !== "object") return false;
  const obj = val as Record<string, unknown>;
  return (
    obj.renderEngine === "diagram_spec" ||
    (Array.isArray(obj.nodes) && Array.isArray(obj.edges) && typeof obj.kind === "string")
  );
}
