import { useState } from "react";
import type { ClassRecord, Concept, ConceptLink } from "../../lib/schemas";
import { RELATION_HINTS, classLabel, describeLink, truncate } from "./graph";
import type { GraphLayout, NodeOption } from "./graph";

export type Selection = { kind: "node"; id: string } | { kind: "link"; id: string } | null;

/** The drawing: one column per week, classes with the concepts the student
 *  wrote beneath them, and the relations as labelled curves. Purely
 *  presentational — nodes and relations are focusable and clickable to select. */
export function GraphCanvas({
  layout,
  selection,
  onSelect,
}: {
  layout: GraphLayout;
  selection: Selection;
  onSelect: (s: Selection) => void;
}) {
  const connected = new Set<string>();
  if (selection?.kind === "link") {
    const edge = layout.edges.find((e) => e.id === selection.id);
    if (edge) [edge.fromId, edge.toId].forEach((id) => connected.add(id));
  }
  return (
    <div className="graph-scroll">
      <svg className="graph-svg" width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} role="group" aria-label="Grafo de ideas">
        {layout.columns.map((c, i) => (
          <text key={i} className="graph-column-label" x={c.x} y={30}>
            {c.label}
          </text>
        ))}
        {layout.edges.map((e) => {
          const selected = selection?.kind === "link" && selection.id === e.id;
          return (
            <g
              key={e.id}
              className={`graph-edge${selected ? " is-selected" : ""}`}
              role="button"
              tabIndex={0}
              aria-label={`Relación: ${e.label}`}
              onClick={() => onSelect({ kind: "link", id: e.id })}
              onKeyDown={(ev) => ev.key === "Enter" && onSelect({ kind: "link", id: e.id })}
            >
              <title>{e.label}</title>
              <path className="graph-edge-hit" d={e.path} />
              <path className="graph-edge-line" d={e.path} markerEnd="url(#graph-arrow)" />
              <text className="graph-edge-label" x={e.labelX} y={e.labelY} textAnchor={e.labelAnchor}>
                {truncate(e.label, 24)}
              </text>
            </g>
          );
        })}
        <defs>
          <marker id="graph-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
            <path d="M 0 0 L 10 5 L 0 10 z" className="graph-arrow-head" />
          </marker>
        </defs>
        {layout.nodes.map((n) => {
          const selected = (selection?.kind === "node" && selection.id === n.id) || connected.has(n.id);
          return (
            <g
              key={n.id}
              className={`graph-node graph-node-${n.kind}${selected ? " is-selected" : ""}`}
              role="button"
              tabIndex={0}
              aria-label={`${n.kind === "class" ? "Clase" : "Concepto"}: ${n.label}`}
              onClick={() => onSelect({ kind: "node", id: n.id })}
              onKeyDown={(ev) => ev.key === "Enter" && onSelect({ kind: "node", id: n.id })}
            >
              <title>{n.label}</title>
              <rect x={n.x} y={n.y} width={n.w} height={n.h} rx={8} />
              <text x={n.x + 12} y={n.y + n.h / 2 + 5}>
                {truncate(n.label, n.kind === "class" ? 28 : 24)}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

/** Every relation as a sentence — the accessible twin of the drawing. */
export function RelationList({
  links,
  options,
  selection,
  onSelect,
}: {
  links: ConceptLink[];
  options: NodeOption[];
  selection: Selection;
  onSelect: (s: Selection) => void;
}) {
  if (links.length === 0) return null;
  return (
    <ul className="relation-list" aria-label="Tus relaciones">
      {links.map((l) => (
        <li key={l.id}>
          <button className={`btn-quiet${selection?.kind === "link" && selection.id === l.id ? " is-selected" : ""}`} onClick={() => onSelect({ kind: "link", id: l.id })}>
            {describeLink(l, options)}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** Writes a concept about a class, in the student's own words. */
export function ConceptForm({
  classes,
  defaultClassId,
  initialLabel = "",
  submitLabel,
  onSubmit,
  onCancel,
}: {
  classes: ClassRecord[];
  defaultClassId: string;
  initialLabel?: string;
  submitLabel: string;
  onSubmit: (classId: string, label: string) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [classId, setClassId] = useState(defaultClassId);
  const [label, setLabel] = useState(initialLabel);
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    const ok = await onSubmit(classId, label);
    setBusy(false);
    if (ok) onCancel();
  };
  return (
    <form
      className="graph-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label>
        Clase
        <select value={classId} onChange={(e) => setClassId(e.target.value)}>
          {classes.map((c) => (
            <option key={c.id} value={c.id}>
              {classLabel(c.title)}
            </option>
          ))}
        </select>
      </label>
      <label>
        Concepto, con tus palabras
        <input value={label} maxLength={120} placeholder="Una idea de esta clase, como tú la explicarías…" onChange={(e) => setLabel(e.target.value)} autoFocus />
      </label>
      <div className="row">
        <button className="btn-primary" type="submit" disabled={busy || !label.trim()}>
          {submitLabel}
        </button>
        <button className="btn-quiet" type="button" onClick={onCancel}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

/** Draws a relation: where it starts, where it goes, and — required — WHY, in
 *  the student's words. The chips only prefill the field. */
export function LinkForm({
  options,
  initialFrom = "",
  initialTo = "",
  initialLabel = "",
  submitLabel,
  onSubmit,
  onCancel,
}: {
  options: NodeOption[];
  initialFrom?: string;
  initialTo?: string;
  initialLabel?: string;
  submitLabel: string;
  onSubmit: (fromId: string, toId: string, label: string) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [from, setFrom] = useState(initialFrom);
  const [to, setTo] = useState(initialTo);
  const [label, setLabel] = useState(initialLabel);
  const [busy, setBusy] = useState(false);
  const valid = !!from && !!to && from !== to && label.trim().length > 0;
  const submit = async () => {
    setBusy(true);
    const ok = await onSubmit(from, to, label);
    setBusy(false);
    if (ok) onCancel();
  };
  const select = (value: string, set: (v: string) => void, name: string) => (
    <select value={value} onChange={(e) => set(e.target.value)} aria-label={name}>
      <option value="">Elige…</option>
      {options.map((o) => (
        <option key={o.id} value={o.id}>
          {o.kind === "class" ? "Clase · " : "↳ "}
          {truncate(o.label, 60)}
        </option>
      ))}
    </select>
  );
  return (
    <form
      className="graph-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label>
        Desde
        {select(from, setFrom, "Desde")}
      </label>
      <label>
        ¿Cómo se relacionan? Con tus palabras
        <input value={label} maxLength={120} placeholder="Por qué o cómo se conectan…" onChange={(e) => setLabel(e.target.value)} />
      </label>
      <div className="chip-row" aria-label="Atajos para escribir la relación">
        {RELATION_HINTS.map((h) => (
          <button key={h} type="button" className="chip" onClick={() => setLabel((l) => (l.trim() ? l : h))}>
            {h}
          </button>
        ))}
      </div>
      <label>
        Hacia
        {select(to, setTo, "Hacia")}
      </label>
      {from && from === to && <p className="hint hint-warn">Elige dos ideas distintas.</p>}
      <div className="row">
        <button className="btn-primary" type="submit" disabled={busy || !valid}>
          {submitLabel}
        </button>
        <button className="btn-quiet" type="button" onClick={onCancel}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

/** A destructive action behind a second click (no `window.confirm`: it blocks
 *  the webview and can't be styled or tested). */
export function ConfirmButton({ label, warning, onConfirm }: { label: string; warning: string; onConfirm: () => void }) {
  const [armed, setArmed] = useState(false);
  if (!armed) {
    return (
      <button className="btn-quiet btn-danger-quiet" onClick={() => setArmed(true)}>
        {label}
      </button>
    );
  }
  return (
    <span className="graph-confirm">
      <span className="hint hint-warn">{warning}</span>
      <button className="btn-quiet btn-danger-quiet" onClick={onConfirm}>
        Sí, borrar
      </button>
      <button className="btn-quiet" onClick={() => setArmed(false)}>
        No
      </button>
    </span>
  );
}

export type { Concept };
