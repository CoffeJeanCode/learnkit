import mermaid from "mermaid";
import { createElement, useEffect, useRef, useState } from "react";
import type { Key } from "react";
import type { StaticVisualSpec, SvgElement, SvgGroup } from "../../../lib/schemas";

// Explicitly static/declarative visuals only (arXiv:2605.30174) — never a
// dynamic simulator. 3 rendering engines, one component each, dispatched by
// `StaticVisual` below.

let mermaidInitialized = false;
function ensureMermaidInit() {
  if (mermaidInitialized) return;
  mermaid.initialize({ startOnLoad: false, theme: "dark", securityLevel: "strict" });
  mermaidInitialized = true;
}

let mermaidRenderCounter = 0;

// Mermaid's `stateDiagram` grammar has no `::` token: a Rust/C++-style path in
// a transition label (`String::from("nota")`) fails to parse — verified against
// mermaid 12, where flowchart/sequenceDiagram accept `::` but stateDiagram does
// not. U+2236 RATIO looks like a colon and lexes as ordinary text, so swap it
// in at render time. The stored source is never modified: this only affects
// what we hand to `mermaid.render`, so a fix here also repairs notebooks that
// were generated before the grounding rule existed.
export function repairMermaid(code: string): string {
  if (!code.includes("::")) return code;
  if (!/^\s*stateDiagram/.test(code)) return code;
  return code.replace(/::/g, "\u2236\u2236");
}

function MermaidVisual({ code, caption }: { code: string; caption: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    ensureMermaidInit();
    let cancelled = false;
    setError(null);
    const id = `mermaid-diagram-${++mermaidRenderCounter}`;
    mermaid
      .render(id, repairMermaid(code))
      .then(({ svg }) => {
        if (!cancelled && containerRef.current) containerRef.current.innerHTML = svg;
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [code]);

  return (
    <figure className="static-visual">
      {error ? (
        <div className="alert error">
          <span className="alert-text">No se pudo renderizar este diagrama (sintaxis que Mermaid no entiende) — la leyenda de abajo explica el concepto.</span>
          <span className="hint">{error}</span>
        </div>
      ) : (
        <div className="mermaid-visual" ref={containerRef} />
      )}
      <figcaption className="hint">{caption}</figcaption>
    </figure>
  );
}

function renderSvgElement(el: SvgElement, key: Key) {
  return el.tag === "text"
    ? createElement("text", { key, ...el.props }, el.label)
    : createElement(el.tag, { key, ...el.props });
}

// Arrowhead markers for the mandated dark-mode palette (see
// `notebook_agent`'s system prompt) — one per palette color, keyed to the
// same convention the model is told to use: `markerEnd="url(#arrow-sky)"`
// etc. Defined once here so a spec never needs to declare its own <defs>.
const ARROW_MARKER_COLORS: Record<string, string> = {
  sky: "#38bdf8",
  green: "#4ade80",
  rose: "#f87171",
  amber: "#fbbf24",
  violet: "#c084fc",
  neutral: "#94a3b8",
};

function ArrowMarkerDefs() {
  return (
    <defs>
      {Object.entries(ARROW_MARKER_COLORS).map(([name, color]) => (
        <marker
          key={name}
          id={`arrow-${name}`}
          viewBox="0 0 10 10"
          refX="8"
          refY="5"
          markerWidth="7"
          markerHeight="7"
          orient="auto-start-reverse"
        >
          <path d="M 0 0 L 10 5 L 0 10 z" fill={color} />
        </marker>
      ))}
    </defs>
  );
}

function DeclarativeSvgVisual({
  viewBox,
  elements,
  groups,
  pedagogicalFocus,
  caption,
}: {
  viewBox: string;
  elements: SvgElement[];
  groups: SvgGroup[];
  pedagogicalFocus?: string;
  caption: string;
}) {
  return (
    <figure className="static-visual">
      <svg viewBox={viewBox} className="declarative-svg-visual">
        <ArrowMarkerDefs />
        {elements.map((el, i) => renderSvgElement(el, i))}
        {groups.map((g) => (
          <g key={g.groupId} data-group-id={g.groupId} data-pedagogical-role={g.pedagogicalRole}>
            {g.elements.map((el, i) => renderSvgElement(el, i))}
          </g>
        ))}
      </svg>
      {pedagogicalFocus && <p className="svg-pedagogical-focus">{pedagogicalFocus}</p>}
      <figcaption className="hint">{caption}</figcaption>
    </figure>
  );
}

function ConceptualMatrixVisual({ headers, rows, contrastFocus }: { headers: string[]; rows: string[][]; contrastFocus: string }) {
  return (
    <figure className="static-visual">
      <table className="conceptual-matrix-visual">
        <thead>
          <tr>
            {headers.map((h) => (
              <th key={h}>{h}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr key={i}>
              {row.map((cell, j) => (
                <td key={j}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <figcaption className="hint">Comparando: {contrastFocus}</figcaption>
    </figure>
  );
}

export function StaticVisual({ visual }: { visual: StaticVisualSpec }) {
  switch (visual.renderEngine) {
    case "mermaid":
      return <MermaidVisual code={visual.code} caption={visual.caption} />;
    case "declarative_svg":
      return (
        <DeclarativeSvgVisual
          viewBox={visual.viewBox}
          elements={visual.elements}
          groups={visual.groups}
          pedagogicalFocus={visual.pedagogicalFocus ?? undefined}
          caption={visual.caption}
        />
      );
    case "conceptual_matrix":
      return <ConceptualMatrixVisual headers={visual.headers} rows={visual.rows} contrastFocus={visual.contrastFocus} />;
  }
}
