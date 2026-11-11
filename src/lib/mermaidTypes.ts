// Which Mermaid chart a piece of source declares — mirrors
// `MermaidChartType::from_code` in Rust (the header is what Mermaid renders by).

import type { MERMAID_CHART_TYPES } from "./schemas";

export type MermaidChartType = (typeof MERMAID_CHART_TYPES)[number];

/** First tokens (lower-cased) Mermaid 12 accepts for each chart type. */
export const MERMAID_HEADERS: Record<MermaidChartType, string[]> = {
  flowchart: ["flowchart", "graph"],
  sequenceDiagram: ["sequencediagram"],
  classDiagram: ["classdiagram"],
  stateDiagram: ["statediagram", "statediagram-v2"],
  erDiagram: ["erdiagram"],
  mindmap: ["mindmap"],
  timeline: ["timeline"],
  gantt: ["gantt"],
  pie: ["pie"],
  quadrantChart: ["quadrantchart"],
  journey: ["journey"],
  gitGraph: ["gitgraph", "gitgraph:"],
  xychart: ["xychart", "xychart-beta"],
  sankey: ["sankey", "sankey-beta"],
};

export function mermaidTypeOf(code: string): MermaidChartType | null {
  const first = code
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l && !l.startsWith("%%"));
  const token = first?.toLowerCase().split(/\s+/)[0];
  if (!token) return null;
  const hit = (Object.entries(MERMAID_HEADERS) as Array<[MermaidChartType, string[]]>).find(([, heads]) => heads.includes(token));
  return hit ? hit[0] : null;
}
