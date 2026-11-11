import { useMemo, useState } from "react";
import { buildGroup } from "../../../lib/algebra";
import type { StaticVisualSpec } from "../../../lib/schemas";

type Spec = Extract<StaticVisualSpec, { renderEngine: "algebraic_structure" }>;

const COSET_COLORS = ["#38bdf8", "#4ade80", "#fbbf24", "#f87171", "#c084fc", "#2dd4bf", "#fb923c", "#a3e635", "#e879f9", "#94a3b8", "#60a5fa", "#facc15"];
const GEN_COLORS = ["#38bdf8", "#f87171"];
const KIND_NAME: Record<string, (n: number) => string> = {
  cyclic: (n) => `ℤ${toSub(n)}`,
  dihedral: (n) => `D${toSub(n)}`,
  symmetric: (n) => `S${toSub(n)}`,
};
function toSub(n: number): string {
  return String(n)
    .split("")
    .map((d) => "₀₁₂₃₄₅₆₇₈₉"[Number(d)])
    .join("");
}

/** A finite group the student explores: the Cayley table and graph are
 *  computed from the group's kind (never written by the model); a subgroup's
 *  left cosets share a colour, so |G| = |H|·[G:H] is visible at a glance. */
export function AlgebraicStructureVisual({ visual }: { visual: Spec }) {
  const { kind, n } = visual.group;
  const g = useMemo(() => buildGroup(kind, n, visual.subgroup), [kind, n, visual.subgroup]);
  const [picked, setPicked] = useState<number | null>(null);
  if (!g) return <p className="hint hint-warn">No se pudo construir este grupo.</p>;

  const order = g.elements.length;
  const hasH = g.subgroup.length > 0;
  const cosets = hasH ? Math.max(...g.coset) + 1 : 0;
  const colorOf = (i: number) => (hasH ? COSET_COLORS[g.coset[i] % COSET_COLORS.length] : "#94a3b8");
  const showTable = visual.view !== "graph" && order <= 12;
  const showGraph = visual.view !== "table";
  const name = (KIND_NAME[kind] ?? ((m: number) => `G${m}`))(n);

  // Cayley graph: elements on a circle, one arrow colour per generator.
  const R = 150;
  const C = 190;
  const at = (i: number): [number, number] => [C + R * Math.cos((2 * Math.PI * i) / order - Math.PI / 2), C + R * Math.sin((2 * Math.PI * i) / order - Math.PI / 2)];
  const indexOf = (p: number[]) => g.elements.findIndex((q) => q.join() === p.join());
  const edges = g.generators.flatMap((gen, gi) => g.elements.map((p, i) => ({ from: i, to: indexOf(p.map((x) => gen[x])), gi })));

  return (
    <figure className="static-visual algebra-visual">
      {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
      <p className="im-readout" aria-live="polite">
        <span className="im-readout-label">
          {name}: |G| = {order}
          {hasH && (
            <>
              {" "}
              · |H| = {g.subgroup.length} · [G : H] = {cosets}
            </>
          )}
        </span>
        {hasH && (
          <span className="im-readout-value">
            {order} = {g.subgroup.length} × {cosets}
          </span>
        )}
        {hasH && <span className="im-hint">Cada color es una clase lateral gH: todas tienen el mismo tamaño que H (Lagrange).</span>}
      </p>
      <div className="algebra-panels">
        {showGraph && (
          <svg viewBox="0 0 380 380" className="algebra-graph" role="img" aria-label={`Grafo de Cayley de ${name}`}>
            <defs>
              {GEN_COLORS.map((c, i) => (
                <marker key={c} id={`alg-arrow-${i}`} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
                  <path d="M0,0 L10,5 L0,10 z" fill={c} />
                </marker>
              ))}
            </defs>
            {edges.map(({ from, to, gi }) => {
              if (to < 0 || from === to) return null;
              const [x1, y1] = at(from);
              const [x2, y2] = at(to);
              const len = Math.hypot(x2 - x1, y2 - y1) || 1;
              const s = 17 / len;
              return (
                <line
                  key={`${gi}-${from}`}
                  x1={x1 + (x2 - x1) * s}
                  y1={y1 + (y2 - y1) * s}
                  x2={x2 - (x2 - x1) * s}
                  y2={y2 - (y2 - y1) * s}
                  stroke={GEN_COLORS[gi % 2]}
                  strokeOpacity={picked === null || picked === from ? 0.85 : 0.15}
                  strokeWidth="1.5"
                  markerEnd={`url(#alg-arrow-${gi % 2})`}
                />
              );
            })}
            {g.elements.map((_, i) => {
              const [x, y] = at(i);
              return (
                <g key={i} role="button" tabIndex={0} className="algebra-node" onClick={() => setPicked((c) => (c === i ? null : i))}>
                  <circle cx={x} cy={y} r="15" fill={colorOf(i)} fillOpacity={g.subgroup.includes(i) ? 1 : 0.45} stroke={picked === i ? "#fff" : "none"} strokeWidth="2" />
                  <text x={x} y={y + 4} textAnchor="middle" fontSize={g.names[i].length > 5 ? 8 : 11} fill="#0b0e13" fontWeight="700">
                    {g.names[i]}
                  </text>
                </g>
              );
            })}
          </svg>
        )}
        {showTable && (
          <table className="algebra-table" aria-label={`Tabla de Cayley de ${name}`}>
            <thead>
              <tr>
                <th>∘</th>
                {g.names.map((nm, j) => (
                  <th key={j} style={{ color: colorOf(j) }}>
                    {nm}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {g.table.map((row, i) => (
                <tr key={i} className={picked === i ? "is-picked" : undefined}>
                  <th style={{ color: colorOf(i) }}>{g.names[i]}</th>
                  {row.map((cell, j) => (
                    <td key={j} style={{ background: `${colorOf(cell)}33` }}>
                      {g.names[cell]}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {showGraph && (
        <p className="hint">
          Flechas <span style={{ color: GEN_COLORS[0] }}>azules</span> = multiplicar por {g.names[g.elements.findIndex((p) => p.join() === g.generators[0].join())]}
          {g.generators[1] && (
            <>
              , <span style={{ color: GEN_COLORS[1] }}>rojas</span> = por {g.names[g.elements.findIndex((p) => p.join() === g.generators[1].join())]}
            </>
          )}
          . {hasH ? "Los nodos opacos forman H." : ""} Toca un elemento para seguir sus flechas.
        </p>
      )}
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
