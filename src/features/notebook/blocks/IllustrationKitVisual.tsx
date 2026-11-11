import { useState } from "react";
import type { ReactNode } from "react";
import { CELL_H, CELL_W, VIEW_H, VIEW_W, directionVector, forceArrow, labelBox, linkPath, slotCentre, trimmed } from "../../../lib/illustration";
import type { StaticVisualSpec } from "../../../lib/schemas";

type Spec = Extract<StaticVisualSpec, { renderEngine: "illustration_kit" }>;
type Item = Spec["items"][number];

const INK = "#cbd5e1";
const FORCE = ["#f87171", "#fbbf24", "#4ade80", "#38bdf8", "#c084fc"];

/** One piece of the kit, centred on (0, 0). */
function Piece({ item }: { item: Item }): ReactNode {
  const s = { stroke: INK, strokeWidth: 2, fill: "#1e293b" };
  switch (item.kind) {
    case "block":
      return <rect x={-24} y={-20} width={48} height={40} rx={3} {...s} fill="#334155" />;
    case "ball":
      return <circle r={20} {...s} fill="#334155" />;
    case "pulley":
      return (
        <g>
          <circle r={18} {...s} />
          <circle r={4} fill={INK} />
          <line x1={0} y1={-18} x2={0} y2={-40} stroke={INK} strokeWidth={2} />
        </g>
      );
    case "incline": {
      const a = ((item.angle ?? 30) * Math.PI) / 180;
      const w = CELL_W - 20;
      const h = Math.min(CELL_H - 10, Math.tan(a) * w);
      return <path d={`M${-w / 2},${CELL_H / 2 - 8} L${w / 2},${CELL_H / 2 - 8} L${-w / 2},${CELL_H / 2 - 8 - h} Z`} {...s} fill="#273449" />;
    }
    case "wall":
      return (
        <g>
          <rect x={-6} y={-CELL_H / 2 + 4} width={12} height={CELL_H - 8} fill="#475569" />
          {Array.from({ length: 6 }, (_, i) => (
            <line key={i} x1={-6} y1={-CELL_H / 2 + 10 + i * 15} x2={-14} y2={-CELL_H / 2 + 18 + i * 15} stroke={INK} />
          ))}
        </g>
      );
    case "ground":
      return (
        <g>
          <line x1={-CELL_W / 2} y1={CELL_H / 2 - 8} x2={CELL_W / 2} y2={CELL_H / 2 - 8} stroke={INK} strokeWidth={2} />
          {Array.from({ length: 8 }, (_, i) => (
            <line key={i} x1={-CELL_W / 2 + 8 + i * 15} y1={CELL_H / 2 - 8} x2={-CELL_W / 2 + i * 15} y2={CELL_H / 2} stroke="#64748b" />
          ))}
        </g>
      );
    case "battery":
      return (
        <g>
          <line x1={-6} y1={-16} x2={-6} y2={16} stroke={INK} strokeWidth={3} />
          <line x1={6} y1={-9} x2={6} y2={9} stroke={INK} strokeWidth={6} />
          <text x={-14} y={-18} fill="#f87171" fontSize={12}>
            +
          </text>
        </g>
      );
    case "resistor":
      return <path d="M-26,0 L-18,0 L-14,-9 L-6,9 L2,-9 L10,9 L14,0 L26,0" fill="none" stroke={INK} strokeWidth={2} />;
    case "bulb":
      return (
        <g>
          <circle r={16} {...s} fill="#3f3a1e" />
          <path d="M-7,4 Q0,-12 7,4" fill="none" stroke="#fbbf24" strokeWidth={2} />
        </g>
      );
    case "switch":
      return (
        <g>
          <circle cx={-14} r={3} fill={INK} />
          <circle cx={14} r={3} fill={INK} />
          <line x1={-14} y1={0} x2={10} y2={-12} stroke={INK} strokeWidth={2} />
        </g>
      );
    case "capacitor":
      return (
        <g>
          <line x1={-5} y1={-14} x2={-5} y2={14} stroke={INK} strokeWidth={3} />
          <line x1={5} y1={-14} x2={5} y2={14} stroke={INK} strokeWidth={3} />
        </g>
      );
    case "magnet":
      return (
        <g>
          <rect x={-24} y={-12} width={24} height={24} fill="#ef4444" />
          <rect x={0} y={-12} width={24} height={24} fill="#3b82f6" />
          <text x={-12} y={5} textAnchor="middle" fontSize={12} fill="#fff">
            N
          </text>
          <text x={12} y={5} textAnchor="middle" fontSize={12} fill="#fff">
            S
          </text>
        </g>
      );
    case "node":
      return <circle r={4} fill={INK} />;
    default:
      return null;
  }
}

/** A physics setup or circuit composed from a closed kit: the model placed
 *  pieces on grid slots and named links and forces; the layout, link routes
 *  and label placement are computed here. Tap a piece to read about it. */
export function IllustrationKitVisual({ visual, highlightGroupId }: { visual: Spec; highlightGroupId?: string }) {
  const [selected, setSelected] = useState<string | null>(null);
  const active = selected ?? (visual.items.find((i) => i.id === highlightGroupId || i.label === highlightGroupId)?.id ?? null);
  const byId = new Map(visual.items.map((i) => [i.id, i]));
  const current = active ? byId.get(active) : undefined;

  return (
    <figure className="static-visual illustration-kit">
      {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
      <svg viewBox={`0 0 ${VIEW_W} ${VIEW_H}`} className="illustration-svg" role="img" aria-label={visual.items.map((i) => i.label).join(", ")}>
        <defs>
          {FORCE.map((c, i) => (
            <marker key={c} id={`ik-arrow-${i}`} viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto">
              <path d="M0,0 L10,5 L0,10 z" fill={c} />
            </marker>
          ))}
          <marker id="ik-arrow-link" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto">
            <path d="M0,0 L10,5 L0,10 z" fill={INK} />
          </marker>
        </defs>
        {visual.links.map((l, i) => {
          const a = byId.get(l.from);
          const b = byId.get(l.to);
          if (!a || !b) return null;
          const [p, q] = l.kind === "wire" ? [slotCentre(a.slot), slotCentre(b.slot)] : trimmed(slotCentre(a.slot), slotCentre(b.slot));
          const d = linkPath(l.kind, p, q);
          const style = l.kind === "rod" ? { stroke: "#94a3b8", strokeWidth: 5 } : l.kind === "wire" ? { stroke: "#fbbf24", strokeWidth: 2 } : { stroke: INK, strokeWidth: 2 };
          return (
            <g key={i}>
              <path d={d} fill="none" {...style} markerEnd={l.kind === "arrow" ? "url(#ik-arrow-link)" : undefined} />
              {l.label && (
                <text x={(p[0] + q[0]) / 2 + 6} y={(p[1] + q[1]) / 2 - 6} fill="#94a3b8" fontSize={12}>
                  {l.label}
                </text>
              )}
            </g>
          );
        })}
        {visual.items.map((it) => {
          const [x, y] = slotCentre(it.slot);
          const box = labelBox(it);
          const dim = active !== null && active !== it.id;
          return (
            <g key={it.id} transform={`translate(${x},${y})`} opacity={dim ? 0.45 : 1} role="button" tabIndex={0} className="ik-piece" onClick={() => setSelected((c) => (c === it.id ? null : it.id))}>
              <Piece item={it} />
              {it.kind !== "node" && it.label && (
                <g transform={`translate(${box.x - x},${box.y - y})`}>
                  <rect width={box.w} height={box.h} rx={4} fill="#0f172a" fillOpacity={0.85} stroke={active === it.id ? "#fbbf24" : "none"} />
                  <text x={box.w / 2} y={13} textAnchor="middle" fontSize={12} fill="#e2e8f0">
                    {it.value ? `${it.label} · ${it.value}` : it.label}
                  </text>
                </g>
              )}
            </g>
          );
        })}
        {visual.forces.map((f, i) => {
          const it = byId.get(f.on);
          if (!it) return null;
          const arrow = forceArrow(it, f.direction, f.magnitude);
          const color = FORCE[i % FORCE.length];
          return (
            <g key={i}>
              <line x1={arrow.x1} y1={arrow.y1} x2={arrow.x2} y2={arrow.y2} stroke={color} strokeWidth={3} markerEnd={`url(#ik-arrow-${i % FORCE.length})`} />
              <text x={arrow.lx} y={arrow.ly} fill={color} fontSize={13} fontWeight={700} textAnchor="middle">
                {f.label}
              </text>
            </g>
          );
        })}
      </svg>
      {current?.explanation ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>{current.label}</strong>
          <p>{current.explanation}</p>
        </aside>
      ) : (
        <p className="hint node-note-hint">Toca una pieza para ver qué es.</p>
      )}
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
