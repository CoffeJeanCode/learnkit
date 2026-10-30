import { type ReactNode, useCallback, useLayoutEffect, useRef, useState } from "react";
import { type Port, dropIndex, moveItem, reorderTo, slideOffset, snapPort, wirePath } from "./interaction";

/* Node-editor interactions (the TouchDesigner vocabulary) for the gates:
 *  · ChainOrder  — every step is a node on a chain; DRAG one and its neighbours
 *    slide aside to open the slot it will land in.
 *  · CaseWiring  — every case is a node with an output port; DRAG a wire from
 *    it to the verdict it belongs to. The wire follows the pointer and snaps to
 *    a port when close; click a wire to unplug it.
 * Wires are always neutral: colour never hints at the right answer (grading is
 * server-side). Both keep a keyboard path (arrows / a radio pair) so nothing is
 * pointer-only. */

const CHAIN_GAP = 18;

export function ChainOrder({
  steps,
  onChange,
  disabled,
  render,
}: {
  steps: readonly string[];
  onChange: (next: string[]) => void;
  disabled?: boolean;
  render: (step: string) => ReactNode;
}) {
  const items = useRef<(HTMLLIElement | null)[]>([]);
  const session = useRef<{ from: number; to: number; startY: number; centres: number[]; slot: number } | null>(null);
  const [drag, setDrag] = useState<{ from: number; to: number; dy: number; slot: number } | null>(null);

  const onDown = (e: React.PointerEvent<HTMLLIElement>, i: number) => {
    if (disabled || e.button !== 0 || (e.target as Element).closest("button")) return;
    const rects = items.current.map((el) => el!.getBoundingClientRect());
    const slot = rects[i]!.height + CHAIN_GAP;
    session.current = { from: i, to: i, startY: e.clientY, centres: rects.map((r) => r.top + r.height / 2), slot };
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ from: i, to: i, dy: 0, slot });
  };
  const onMove = (e: React.PointerEvent) => {
    const s = session.current;
    if (!s) return;
    const dy = e.clientY - s.startY;
    s.to = dropIndex(s.centres, s.from, s.centres[s.from]! + dy);
    setDrag({ from: s.from, to: s.to, dy, slot: s.slot });
  };
  const end = (commit: boolean) => {
    const s = session.current;
    session.current = null;
    // `s.to` is kept in the ref: a fast drag can release before React re-renders.
    if (s && commit && s.to !== s.from) onChange(reorderTo(steps, s.from, s.to));
    setDrag(null);
  };
  const onKey = (e: React.KeyboardEvent, i: number) => {
    if (disabled || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    onChange(moveItem(steps, i, e.key === "ArrowUp" ? -1 : 1));
  };

  return (
    <ol className={`chain-list${drag ? " is-dragging" : ""}`} aria-label="Cadena de eslabones">
      {steps.map((step, i) => {
        const dragging = drag?.from === i;
        const shift = drag ? slideOffset(i, drag.from, drag.to, drag.slot) : 0;
        return (
          <li
            key={step}
            ref={(el) => {
              items.current[i] = el;
            }}
            className={`chain-item${dragging ? " is-dragged" : ""}`}
            style={{ transform: dragging ? `translateY(${drag!.dy}px)` : shift ? `translateY(${shift}px)` : undefined }}
            onPointerDown={(e) => onDown(e, i)}
            onPointerMove={onMove}
            onPointerUp={() => end(true)}
            onPointerCancel={() => end(false)}
            tabIndex={disabled ? -1 : 0}
            onKeyDown={(e) => onKey(e, i)}
            aria-label={`Paso ${i + 1} de ${steps.length}. Flechas arriba y abajo para moverlo.`}
          >
            <span className="node-port node-port-in" aria-hidden="true" />
            <span className="chain-no">{i + 1}</span>
            <span className="chain-text">{render(step)}</span>
            <span className="chain-grip" aria-hidden="true">
              ⋮⋮
            </span>
            <span className="node-port node-port-out" aria-hidden="true" />
          </li>
        );
      })}
    </ol>
  );
}

export interface WiringCase {
  key: string;
  content: ReactNode;
}

const SNAP_RADIUS = 44;
const SINKS = [
  { id: "true", label: "Cumple la regla" },
  { id: "false", label: "Rompe la regla" },
] as const;

type Point = { x: number; y: number };

export function CaseWiring({
  cases,
  verdicts,
  onChange,
  disabled,
}: {
  cases: readonly WiringCase[];
  verdicts: readonly (boolean | null)[];
  onChange: (index: number, verdict: boolean | null) => void;
  disabled?: boolean;
}) {
  const box = useRef<HTMLDivElement>(null);
  const outRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const inRefs = useRef<Record<string, HTMLSpanElement | null>>({});
  // Latest sink positions, readable inside pointer handlers without waiting for a re-render.
  const geo = useRef<{ ins: Port[] }>({ ins: [] });
  const [outs, setOuts] = useState<Point[]>([]);
  const [ins, setIns] = useState<Port[]>([]);
  const [drag, setDragState] = useState<{ from: number; at: Point; snap: string | null } | null>(null);
  // The latest drag lives in a ref too: a fast drag can release before React re-renders.
  const dragRef = useRef<{ from: number; at: Point; snap: string | null } | null>(null);
  const setDrag = (next: { from: number; at: Point; snap: string | null } | null) => {
    dragRef.current = next;
    setDragState(next);
  };

  const measure = useCallback(() => {
    const b = box.current?.getBoundingClientRect();
    if (!b) return;
    const centre = (el: Element | null): Point => {
      const r = el!.getBoundingClientRect();
      return { x: r.left + r.width / 2 - b.left, y: r.top + r.height / 2 - b.top };
    };
    const nextOuts = outRefs.current.slice(0, cases.length).map((el) => centre(el));
    const nextIns = SINKS.map((s) => ({ id: s.id, ...centre(inRefs.current[s.id] ?? null) }));
    geo.current = { ins: nextIns };
    setOuts(nextOuts);
    setIns(nextIns);
  }, [cases.length]);

  // Port positions only change with the layout: measure on mount and on resize.
  useLayoutEffect(() => {
    measure();
    // A web font that arrives late changes label widths without resizing the box.
    void document.fonts?.ready.then(measure);
    const el = box.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [measure]);

  const local = (e: React.PointerEvent): Point => {
    const b = box.current!.getBoundingClientRect();
    return { x: e.clientX - b.left, y: e.clientY - b.top };
  };
  const onDown = (e: React.PointerEvent<HTMLButtonElement>, i: number) => {
    if (disabled || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    measure(); // the layout may have shifted since mount: snap against where the ports are NOW
    const at = local(e);
    setDrag({ from: i, at, snap: snapPort(geo.current.ins, at.x, at.y, SNAP_RADIUS)?.id ?? null });
  };
  const onMove = (e: React.PointerEvent) => {
    const current = dragRef.current;
    if (!current) return;
    const at = local(e);
    setDrag({ from: current.from, at, snap: snapPort(geo.current.ins, at.x, at.y, SNAP_RADIUS)?.id ?? null });
  };
  const onUp = () => {
    // Dropping away from a port cancels: the wire retracts and nothing changes.
    const current = dragRef.current;
    if (current?.snap) onChange(current.from, current.snap === "true");
    setDrag(null);
  };

  const sink = (v: boolean | null): Port | undefined => (v === null ? undefined : ins.find((p) => p.id === String(v)));

  return (
    <div className={`wiring${drag ? " is-wiring" : ""}`} ref={box}>
      <svg className="wiring-layer" aria-hidden="true">
        {cases.map((c, i) => {
          const from = outs[i];
          const to = sink(verdicts[i] ?? null);
          if (!from || !to || drag?.from === i) return null;
          return (
            <g key={c.key} className="wire" onClick={() => !disabled && onChange(i, null)}>
              <path className="wire-hit" d={wirePath(from.x, from.y, to.x, to.y)} />
              <path className="wire-line" d={wirePath(from.x, from.y, to.x, to.y)} />
              <title>Clic para desconectar</title>
            </g>
          );
        })}
        {drag && outs[drag.from] && (
          <path
            className={`wire-line wire-live${drag.snap ? " is-snapped" : ""}`}
            d={(() => {
              const end = drag.snap ? ins.find((p) => p.id === drag.snap)! : drag.at;
              return wirePath(outs[drag.from]!.x, outs[drag.from]!.y, end.x, end.y);
            })()}
          />
        )}
      </svg>

      <ul className="wiring-cases">
        {cases.map((c, i) => (
          <li key={c.key} className="wire-node">
            <div className="wire-text">{c.content}</div>
            <button
              type="button"
              ref={(el) => {
                outRefs.current[i] = el;
              }}
              className={`node-port node-port-out wire-handle${verdicts[i] !== null ? " is-linked" : ""}`}
              onPointerDown={(e) => onDown(e, i)}
              onPointerMove={onMove}
              onPointerUp={onUp}
              onPointerCancel={() => setDrag(null)}
              disabled={disabled}
              aria-label={`Arrastra desde el caso ${i + 1} hasta «Cumple la regla» o «Rompe la regla»`}
              tabIndex={-1}
            />
            {/* Keyboard / screen-reader path: the same choice as a radio pair, shown while focused. */}
            <div className="kbd-fallback" role="radiogroup" aria-label={`Caso ${i + 1}`}>
              {([true, false] as const).map((value) => (
                <button key={String(value)} type="button" role="radio" aria-checked={verdicts[i] === value} className={verdicts[i] === value ? "btn-primary" : "btn-quiet"} onClick={() => onChange(i, value)} disabled={disabled}>
                  {value ? "Cumple la regla" : "Rompe la regla"}
                </button>
              ))}
            </div>
          </li>
        ))}
      </ul>

      <div className="wiring-sinks">
        {SINKS.map((s) => {
          const linked = verdicts.filter((v) => String(v) === s.id).length;
          return (
            <div key={s.id} className={`wire-sink${drag?.snap === s.id ? " is-snapped" : ""}`}>
              <span
                ref={(el) => {
                  inRefs.current[s.id] = el;
                }}
                className="node-port node-port-in"
                aria-hidden="true"
              />
              <span className="wire-sink-label">{s.label}</span>
              <span className="wire-sink-count">{linked}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
