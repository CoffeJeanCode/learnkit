import mermaid from "mermaid";
import { createElement, useEffect, useRef, useState } from "react";
import type { Key } from "react";
import type { StaticVisualSpec, SvgElement, SvgGroup } from "../../../lib/schemas";
import { compileDiagramSpecToMermaid } from "../../../lib/diagramSpec";
import { InteractiveModuleVisual } from "./InteractiveModule";

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
  let fixed = (code || "").trim();

  // 1. Strip markdown fences if present
  fixed = fixed.replace(/^\`\`\`(?:mermaid)?\s*\r?\n?/i, "").replace(/\r?\n?\`\`\`\s*$/i, "").trim();

  // 2. Fix :: in state diagrams
  if (fixed.includes("::") && /^\s*stateDiagram/.test(fixed)) {
    fixed = fixed.replace(/::/g, "\u2236\u2236");
  }

  // 3. Auto-quote pipe edge labels: -->|text| => -->|"text"|
  fixed = fixed.replace(/(-->|-\.->|==>)\|([^"|\n]+)\|/g, (_m, arrow, label) => {
    return `${arrow}|"${label.replace(/"/g, "'")}"|`;
  });

  // 4. Auto-quote rectangle node labels: A[text without leading quote] => A["text"]
  // Exclude cylinders [("...")] and subroutines [["..."]]
  fixed = fixed.replace(/(?<=^|[\s;])([A-Za-z0-9_-]+)\[(?!["(\[])([^"\n\]]+)\]/gm, (_m, id, label) => {
    return `${id}["${label.replace(/"/g, "'")}"]`;
  });

  // 5. Auto-quote diamond labels: A{text} => A{"text"}
  fixed = fixed.replace(/(?<=^|[\s;])([A-Za-z0-9_-]+)\{(?!")([^"\n\}]+)\}/gm, (_m, id, label) => {
    return `${id}{"${label.replace(/"/g, "'")}"}`;
  });

  // 6. Auto-quote rounded node labels: A(text) => A("text")
  // Exclude circle ((...))
  fixed = fixed.replace(/(?<=^|[\s;])([A-Za-z0-9_-]+)\((?!["(])([^"\n\)]+)\)/gm, (_m, id, label) => {
    return `${id}("${label.replace(/"/g, "'")}")`;
  });

  return fixed;
}

/** Node id of a rendered Mermaid node element (`flowchart-cache-3` → `cache`). */
const MERMAID_NODE_ID = /^(?:flowchart|state|graph)-(.+)-\d+$/;

interface NodeNote {
  label: string;
  explanation: string;
}

function MermaidVisual({
  code,
  caption,
  notes,
  highlight,
}: {
  code: string;
  caption: string;
  /** node id → tap-to-learn note (diagram_spec `explanation`). */
  notes?: Record<string, NodeNote>;
  /** node id or label to spotlight (a guided-walkthrough step); others dim. */
  highlight?: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [rendered, setRendered] = useState(0);
  const [openId, setOpenId] = useState<string | null>(null);
  const hasNotes = !!notes && Object.keys(notes).length > 0;
  // A fresh `notes` object arrives every render; key the effect on its content.
  const notesRef = useRef(notes);
  notesRef.current = notes;
  const notesKey = notes ? JSON.stringify(notes) : "";

  useEffect(() => {
    ensureMermaidInit();
    let cancelled = false;
    setError(null);
    const id = `mermaid-diagram-${++mermaidRenderCounter}`;
    mermaid
      .render(id, repairMermaid(code))
      .then(({ svg }) => {
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = svg;
          setRendered((n) => n + 1);
        }
      })
      .catch((e: unknown) => {
        // Remove Mermaid's injected error element from document.body to prevent DOM clutter
        const stray = document.getElementById(`d${id}`);
        if (stray) stray.remove();

        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [code]);

  // Make nodes tappable / spotlight one. DOM-only (the SVG is Mermaid's), run
  // after every render of the SVG and every change of what is highlighted.
  useEffect(() => {
    const root = containerRef.current;
    if (!root) return;
    const cleanups: Array<() => void> = [];
    const needle = highlight?.trim().toLowerCase();
    let matched = false;
    const nodes = Array.from(root.querySelectorAll<SVGGElement>("g.node"));
    const idOf = (el: Element) => MERMAID_NODE_ID.exec(el.id)?.[1] ?? null;
    if (needle) {
      matched = nodes.some((el) => {
        const id = idOf(el);
        return (id && id.toLowerCase() === needle) || (el.textContent ?? "").trim().toLowerCase() === needle;
      });
    }
    for (const el of nodes) {
      const id = idOf(el);
      const isMatch = !!needle && ((id && id.toLowerCase() === needle) || (el.textContent ?? "").trim().toLowerCase() === needle);
      el.classList.toggle("is-dim", matched && !isMatch);
      el.classList.toggle("is-active", matched && isMatch);
      const note = id ? notesRef.current?.[id] : undefined;
      if (hasNotes && id && note) {
        el.classList.add("is-tappable");
        el.setAttribute("tabindex", "0");
        el.setAttribute("role", "button");
        el.setAttribute("aria-label", `${note.label}: ver explicación`);
        const open = () => setOpenId((cur) => (cur === id ? null : id));
        const key = (e: Event) => {
          const k = (e as KeyboardEvent).key;
          if (k === "Enter" || k === " ") {
            e.preventDefault();
            open();
          }
        };
        el.addEventListener("click", open);
        el.addEventListener("keydown", key);
        cleanups.push(() => {
          el.removeEventListener("click", open);
          el.removeEventListener("keydown", key);
        });
      }
    }
    return () => cleanups.forEach((fn) => fn());
  }, [rendered, notesKey, highlight, hasNotes]);

  const open = openId && notes ? notes[openId] : null;

  return (
    <figure className="static-visual">
      {error ? (
        <div className="alert error">
          <span className="alert-text">No se pudo renderizar este diagrama (sintaxis que Mermaid no entiende) — la leyenda de abajo explica el concepto.</span>
          <span className="hint">{error}</span>
        </div>
      ) : (
        <div className={`mermaid-visual${hasNotes ? " has-notes" : ""}`} ref={containerRef} />
      )}
      {hasNotes && !open && !error && <p className="hint node-note-hint">Toca una pieza del diagrama para ver qué hace.</p>}
      {open && (
        <aside className="node-note" role="note">
          <strong>{open.label}</strong>
          <p>{open.explanation}</p>
          <button type="button" className="btn-quiet" onClick={() => setOpenId(null)}>
            Cerrar
          </button>
        </aside>
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
  highlightGroupId,
}: {
  viewBox: string;
  elements: SvgElement[];
  groups: SvgGroup[];
  pedagogicalFocus?: string;
  caption: string;
  /** Group to spotlight while a walkthrough step is active; every other group
   *  is dimmed. Ignored when no group has that id, so a step that names a
   *  label rather than a group never blanks the figure. */
  highlightGroupId?: string;
}) {
  const spotlight = highlightGroupId && groups.some((g) => g.groupId === highlightGroupId) ? highlightGroupId : null;
  return (
    <figure className="static-visual">
      <svg viewBox={viewBox} className="declarative-svg-visual">
        <ArrowMarkerDefs />
        {elements.map((el, i) => renderSvgElement(el, i))}
        {groups.map((g) => (
          <g
            key={g.groupId}
            data-group-id={g.groupId}
            data-pedagogical-role={g.pedagogicalRole}
            className={spotlight ? (g.groupId === spotlight ? "svg-group is-active" : "svg-group is-dim") : "svg-group"}
          >
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

/** `locked` freezes interactive controls (a prediction gate wants the answer
 *  BEFORE the student can experiment); static engines ignore it. */
export function StaticVisual({ visual, highlightGroupId, locked = false }: { visual: StaticVisualSpec; highlightGroupId?: string; locked?: boolean }) {
  switch (visual.renderEngine) {
    case "mermaid":
      return <MermaidVisual code={visual.code} caption={visual.caption} highlight={highlightGroupId} />;
    case "declarative_svg":
      return (
        <DeclarativeSvgVisual
          viewBox={visual.viewBox}
          elements={visual.elements}
          groups={visual.groups}
          pedagogicalFocus={visual.pedagogicalFocus ?? undefined}
          caption={visual.caption}
          highlightGroupId={highlightGroupId}
        />
      );
    case "conceptual_matrix":
      return <ConceptualMatrixVisual headers={visual.headers} rows={visual.rows} contrastFocus={visual.contrastFocus} />;
    case "diagram_spec": {
      const code = compileDiagramSpecToMermaid({
        kind: visual.kind,
        direction: visual.direction ?? undefined,
        title: visual.title ?? undefined,
        nodes: visual.nodes.map((n) => ({
          id: n.id,
          label: n.label,
          shape: n.shape ?? undefined,
          subtext: n.subtext ?? undefined,
        })),
        edges: visual.edges.map((e) => ({
          from: e.from,
          to: e.to,
          label: e.label ?? undefined,
          style: e.style ?? undefined,
        })),
      });
      const notes: Record<string, NodeNote> = {};
      for (const n of visual.nodes) if (n.explanation?.trim()) notes[n.id] = { label: n.label, explanation: n.explanation.trim() };
      return <MermaidVisual code={code} caption={visual.caption ?? visual.title ?? ""} notes={notes} highlight={highlightGroupId} />;
    }
    case "interactive_module":
      return <InteractiveModuleVisual spec={visual.spec} challenge={visual.challenge} caption={visual.caption} locked={locked} />;
  }
}
