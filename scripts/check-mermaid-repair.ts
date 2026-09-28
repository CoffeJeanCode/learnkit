import { Window } from "happy-dom";

// mermaid -> dompurify needs a real DOM before it is imported, so the
// globals go up first and every module below is pulled in dynamically.
const win = new Window({ url: "https://localhost/" });
const g = globalThis as Record<string, unknown>;
for (const key of [
  "window",
  "document",
  "navigator",
  "location",
  "Node",
  "Element",
  "HTMLElement",
  "SVGElement",
  "Document",
  "DocumentFragment",
  "DOMParser",
  "NodeFilter",
  "MutationObserver",
  "XMLSerializer",
  "getComputedStyle",
  "requestAnimationFrame",
  "cancelAnimationFrame",
]) {
  if (g[key] === undefined && (win as unknown as Record<string, unknown>)[key] !== undefined) {
    g[key] = (win as unknown as Record<string, unknown>)[key];
  }
}

const { repairMermaid } = await import("../src/features/notebook/blocks/StaticVisual");
const mermaid = (await import("mermaid")).default;

// The diagram that shipped broken in a real notebook (block
// b40f0263-932f-43c6-be41-1f2b23fcd826): `stateDiagram` has no `::` token, so
// mermaid.render threw "Parse error on line 2".
const stored = [
  "stateDiagram-v2",
  '    [*] --> Valido: let nota = String::from("idea")',
  "    Valido --> Movido: la pasas a una función por valor",
  "    Valido --> Prestado: creas una referencia con &",
  "    Prestado --> Valido: el préstamo termina",
  "    Movido --> [*]: se libera al salir del scope",
].join("\n");

mermaid.initialize({ startOnLoad: false, theme: "dark", securityLevel: "strict" });

const checks: Record<string, boolean> = {};

const repaired = repairMermaid(stored);
checks["stateDiagram :: gets repaired"] = repaired !== stored && !repaired.includes("::");
checks["only :: changes"] = repaired.replace(/\u2236\u2236/g, "::") === stored;

const other = "flowchart LR\n    A[usa String::from] --> B[ok]";
checks["flowchart is left alone"] = repairMermaid(other) === other;

const clean = "stateDiagram-v2\n    [*] --> Valido: el valor queda prestado";
checks["clean stateDiagram is left alone"] = repairMermaid(clean) === clean;

try {
  await mermaid.parse(stored);
  checks["original diagram is rejected"] = false;
} catch {
  checks["original diagram is rejected"] = true;
}

try {
  await mermaid.parse(repaired);
  checks["repaired diagram parses"] = true;
} catch (e) {
  console.log("repaired parse error:", e instanceof Error ? e.message : String(e));
  checks["repaired diagram parses"] = false;
}

try {
  await mermaid.parse(other);
  checks["flowchart with :: parses"] = true;
} catch (e) {
  console.log("flowchart parse error:", e instanceof Error ? e.message : String(e));
  checks["flowchart with :: parses"] = false;
}

const failed = Object.entries(checks).filter(([, ok]) => !ok);
for (const [name, ok] of Object.entries(checks)) console.log(`${ok ? "[ok]" : "[!!]"} ${name}`);
console.log(failed.length === 0 ? "\nPASS" : `\nFAIL (${failed.length})`);
process.exit(failed.length === 0 ? 0 : 1);
