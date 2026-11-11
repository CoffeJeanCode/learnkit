import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { InteractiveModuleSpec, ModuleChallenge, ModuleVariable } from "../../../lib/schemas";
import { mulberry32 } from "../../../lib/rng";
import { integrate, riemannSum } from "../../../lib/calculus";
import { type CompiledExpr, tryCompileExpr } from "../../../lib/safeExpr";

// Interactive modules (OpenAI-style): the model only fills a small typed spec;
// the maths and the drawing live HERE, so the visual is right by construction.
// The backend already rejected broken formulas (`domain::interactive_module`),
// but every path below still degrades to a message instead of throwing.

const COLORS = ["#38bdf8", "#4ade80", "#fbbf24", "#c084fc"];
const RESULT = "#f87171";
const NEUTRAL = "#94a3b8";
const GRID = 9;

/** 4 significant digits, no trailing noise; "—" for non-finite. */
export function fmt(n: number): string {
  if (!Number.isFinite(n)) return "—";
  if (n === 0) return "0";
  const abs = Math.abs(n);
  if (abs >= 10000 || abs < 0.001) return n.toExponential(2);
  return String(Number(n.toPrecision(4)));
}

function stepOf(v: ModuleVariable): number {
  if (v.step && v.step > 0) return v.step;
  const raw = (v.max - v.min) / 100;
  const pow = Math.pow(10, Math.floor(Math.log10(raw || 1)));
  return pow || 0.01;
}

function initialValues(vars: ModuleVariable[]): Record<string, number> {
  return Object.fromEntries(vars.map((v) => [v.sym, v.init]));
}

function safeEval(expr: CompiledExpr | null, env: Record<string, number>): number {
  if (!expr) return NaN;
  try {
    return expr.eval(env);
  } catch {
    return NaN;
  }
}

export function Slider({ v, value, onChange, disabled, color }: { v: ModuleVariable; value: number; onChange: (n: number) => void; disabled: boolean; color?: string }) {
  return (
    <label className="im-slider" style={color ? ({ "--im-accent": color } as CSSProperties) : undefined}>
      <span className="im-slider-head">
        <span>{v.label}</span>
        <output>
          {fmt(value)}
          {v.unit ? ` ${v.unit}` : ""}
        </output>
      </span>
      <input type="range" min={v.min} max={v.max} step={stepOf(v)} value={value} disabled={disabled} onChange={(e) => onChange(Number(e.target.value))} aria-label={v.label} />
    </label>
  );
}

function Challenge({ challenge, current, range, locked }: { challenge: ModuleChallenge; current: number; range: number; locked: boolean }) {
  if (locked) return null;
  const tol = challenge.tolerance > 0 ? challenge.tolerance : Math.abs(range) * 0.02;
  const done = Number.isFinite(current) && Math.abs(current - challenge.target) <= tol;
  return (
    <p className={`im-challenge${done ? " is-done" : ""}`} role="status">
      <span aria-hidden="true">{done ? "✓" : "🎯"}</span> {challenge.prompt}
      {done && <strong> ¡Lo lograste!</strong>}
    </p>
  );
}

function Frame({ children, locked }: { children: ReactNode; locked: boolean }) {
  return (
    <div className={`im-frame${locked ? " is-locked" : ""}`}>
      {locked && <p className="im-lock-note">Primero haz tu predicción abajo. Después podrás mover los controles y comprobarla.</p>}
      {children}
    </div>
  );
}

// --- relation_explorer -----------------------------------------------------

type Spec<M extends InteractiveModuleSpec["module"]> = Extract<InteractiveModuleSpec, { module: M }>;

function RelationExplorer({ spec, challenge, locked }: { spec: Spec<"relation_explorer">; challenge?: ModuleChallenge | null; locked: boolean }) {
  const expr = useMemo(() => tryCompileExpr(spec.output.expr), [spec.output.expr]);
  const [vals, setVals] = useState(() => initialValues(spec.variables));
  const value = safeEval(expr, vals);

  // Range of the output across all sliders (bounded grid) — scales the gauge.
  const [lo, hi] = useMemo(() => {
    let min = Infinity;
    let max = -Infinity;
    let envs: Record<string, number>[] = [{}];
    for (const v of spec.variables) {
      const pts = Array.from({ length: GRID }, (_, i) => v.min + ((v.max - v.min) * i) / (GRID - 1));
      envs = envs.flatMap((e) => pts.map((p) => ({ ...e, [v.sym]: p })));
    }
    for (const e of envs) {
      const r = safeEval(expr, e);
      if (Number.isFinite(r)) {
        min = Math.min(min, r);
        max = Math.max(max, r);
      }
    }
    return [min, max] as const;
  }, [expr, spec.variables]);

  // Curve of the output against the FIRST slider, the others held where they are.
  const first = spec.variables[0];
  const curve = useMemo(() => {
    if (!first) return [];
    return Array.from({ length: 48 }, (_, i) => {
      const x = first.min + ((first.max - first.min) * i) / 47;
      return { x, y: safeEval(expr, { ...vals, [first.sym]: x }) };
    }).filter((p) => Number.isFinite(p.y));
  }, [expr, first, vals]);

  if (!expr) return <p className="hint hint-warn">No se pudo calcular esta relación.</p>;

  const W = 320;
  const H = 150;
  const P = 22;
  const ys = curve.map((p) => p.y);
  const marker = Number.isFinite(value) ? [value] : [];
  const yMin = Math.min(...ys, ...marker);
  const yMax = Math.max(...ys, ...marker);
  const ySpan = yMax - yMin || 1;
  const sx = (x: number) => P + ((x - first.min) / (first.max - first.min)) * (W - 2 * P);
  const sy = (y: number) => H - P - ((y - yMin) / ySpan) * (H - 2 * P);
  const gauge = hi > lo ? Math.min(1, Math.max(0, (value - lo) / (hi - lo))) : 0;
  const cur = challenge ? (challenge.sym === spec.output.sym ? value : vals[challenge.sym]) : NaN;
  const chRange = challenge ? (challenge.sym === spec.output.sym ? hi - lo : (spec.variables.find((v) => v.sym === challenge.sym)?.max ?? 0) - (spec.variables.find((v) => v.sym === challenge.sym)?.min ?? 0)) : 0;

  return (
    <Frame locked={locked}>
      <div className="im-readout" aria-live="polite">
        <span className="im-readout-label">{spec.output.label}</span>
        <span className="im-readout-value">
          {fmt(value)}
          {spec.output.unit ? <small> {spec.output.unit}</small> : null}
        </span>
        <span className="im-gauge" aria-hidden="true">
          <span className="im-gauge-fill" style={{ width: `${gauge * 100}%` }} />
        </span>
      </div>
      {curve.length > 1 && (
        <svg viewBox={`0 0 ${W} ${H}`} className="im-plot" role="img" aria-label={`${spec.output.label} según ${first.label}`}>
          <line x1={P} y1={H - P} x2={W - P} y2={H - P} stroke={NEUTRAL} strokeWidth="1" />
          <line x1={P} y1={P} x2={P} y2={H - P} stroke={NEUTRAL} strokeWidth="1" />
          <text x={W - P} y={H - 6} fill={NEUTRAL} fontSize="11" textAnchor="end">{first.label}</text>
          <text x={P + 4} y={P - 6} fill={NEUTRAL} fontSize="11">{spec.output.label}</text>
          <polyline fill="none" stroke={COLORS[0]} strokeWidth="2.5" points={curve.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(" ")} />
          <line x1={sx(vals[first.sym])} y1={sy(value)} x2={sx(vals[first.sym])} y2={H - P} stroke={NEUTRAL} strokeDasharray="3 3" />
          <circle cx={sx(vals[first.sym])} cy={sy(value)} r="5.5" fill={RESULT} />
        </svg>
      )}
      <div className="im-sliders">
        {spec.variables.map((v, i) => (
          <Slider key={v.sym} v={v} value={vals[v.sym]} color={COLORS[i % COLORS.length]} disabled={locked} onChange={(n) => setVals((s) => ({ ...s, [v.sym]: n }))} />
        ))}
      </div>
      {challenge && <Challenge challenge={challenge} current={cur} range={chRange} locked={locked} />}
    </Frame>
  );
}

// --- function_plot ---------------------------------------------------------

function FunctionPlot({ spec, challenge, locked }: { spec: Spec<"function_plot">; challenge?: ModuleChallenge | null; locked: boolean }) {
  const expr = useMemo(() => tryCompileExpr(spec.expr), [spec.expr]);
  const [lo, hi] = spec.domain;
  const [vals, setVals] = useState(() => initialValues(spec.params));
  const [x0, setX0] = useState((lo + hi) / 2);
  const area = spec.area && spec.area.from >= lo && spec.area.from < hi ? spec.area : null;
  // The upper limit the student drags; starts most of the way across.
  const [upper, setUpper] = useState(() => (area ? area.from + (hi - area.from) * 0.6 : hi));
  const [rects, setRects] = useState(6);
  const f = (x: number) => safeEval(expr, { ...vals, x });

  const pts = Array.from({ length: 160 }, (_, i) => {
    const x = lo + ((hi - lo) * i) / 159;
    return { x, y: f(x) };
  }).filter((p) => Number.isFinite(p.y) && Math.abs(p.y) < 1e6);

  if (!expr || pts.length < 2) return <p className="hint hint-warn">No se pudo dibujar esta función.</p>;

  // The integral, computed here: A(b) = ∫ f from `area.from` to b.
  const exact = area ? integrate(f, area.from, upper) : NaN;
  const sum = area?.riemann ? riemannSum(f, area.from, upper, rects) : null;
  const accumulated = area?.showAccumulated
    ? Array.from({ length: 80 }, (_, i) => {
        const x = area.from + ((hi - area.from) * i) / 79;
        return { x, y: integrate(f, area.from, x, 60) };
      }).filter((p) => Number.isFinite(p.y))
    : [];

  const W = 360;
  const H = 230;
  const P = 26;
  const yMin0 = Math.min(...pts.map((p) => p.y), ...accumulated.map((p) => p.y), area ? 0 : Infinity);
  const yMax0 = Math.max(...pts.map((p) => p.y), ...accumulated.map((p) => p.y), area ? 0 : -Infinity);
  const pad = (yMax0 - yMin0 || 1) * 0.1;
  const yMin = yMin0 - pad;
  const yMax = yMax0 + pad;
  const sx = (x: number) => P + ((x - lo) / (hi - lo)) * (W - 2 * P);
  const sy = (y: number) => H - P - ((y - yMin) / (yMax - yMin)) * (H - 2 * P);
  const axisY = yMin <= 0 && yMax >= 0 ? sy(0) : H - P;
  const axisX = lo <= 0 && hi >= 0 ? sx(0) : P;

  const h = (hi - lo) / 1000;
  const y0 = f(x0);
  const slope = (f(x0 + h) - f(x0 - h)) / (2 * h);
  const span = (hi - lo) / 4;
  const tangent = spec.tangent && Number.isFinite(slope) && Number.isFinite(y0);
  const cur = challenge ? vals[challenge.sym] : NaN;
  const chParam = challenge ? spec.params.find((p) => p.sym === challenge.sym) : undefined;

  return (
    <Frame locked={locked}>
      <svg viewBox={`0 0 ${W} ${H}`} className="im-plot" role="img" aria-label={`Gráfica de y = ${spec.expr}`}>
        <line x1={P} y1={axisY} x2={W - P} y2={axisY} stroke={NEUTRAL} strokeWidth="1" />
        <line x1={axisX} y1={P} x2={axisX} y2={H - P} stroke={NEUTRAL} strokeWidth="1" />
        <text x={W - P} y={axisY - 6} fill={NEUTRAL} fontSize="11" textAnchor="end">x</text>
        <text x={axisX + 6} y={P} fill={NEUTRAL} fontSize="11">y</text>
        {area && !sum && (
          <polygon
            fill={COLORS[0]}
            fillOpacity="0.28"
            points={[
              `${sx(area.from).toFixed(1)},${sy(0).toFixed(1)}`,
              ...pts.filter((p) => p.x >= area.from && p.x <= upper).map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`),
              `${sx(upper).toFixed(1)},${sy(f(upper)).toFixed(1)}`,
              `${sx(upper).toFixed(1)},${sy(0).toFixed(1)}`,
            ].join(" ")}
          />
        )}
        {sum?.rects.map(([a, b, y], i) =>
          Number.isFinite(y) ? (
            <rect key={i} x={sx(a)} width={Math.max(0.5, sx(b) - sx(a))} y={Math.min(sy(y), sy(0))} height={Math.abs(sy(y) - sy(0))} fill={COLORS[2]} fillOpacity="0.35" stroke={COLORS[2]} strokeWidth="1" />
          ) : null,
        )}
        {accumulated.length > 1 && (
          <polyline fill="none" stroke={COLORS[3]} strokeWidth="2" strokeDasharray="5 4" points={accumulated.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(" ")} />
        )}
        {area && (
          <>
            <line x1={sx(upper)} y1={P} x2={sx(upper)} y2={H - P} stroke={COLORS[2]} strokeWidth="1.5" strokeDasharray="3 3" />
            {accumulated.length > 1 && Number.isFinite(exact) && <circle cx={sx(upper)} cy={sy(exact)} r="5" fill={COLORS[3]} />}
          </>
        )}
        <polyline fill="none" stroke={COLORS[0]} strokeWidth="2.5" points={pts.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(" ")} />
        {tangent && (
          <>
            <line
              x1={sx(x0 - span)}
              y1={sy(y0 - slope * span)}
              x2={sx(x0 + span)}
              y2={sy(y0 + slope * span)}
              stroke={RESULT}
              strokeWidth="2"
              strokeDasharray="6 4"
            />
            <circle cx={sx(x0)} cy={sy(y0)} r="5.5" fill={COLORS[2]} />
          </>
        )}
      </svg>
      {tangent && (
        <div className="im-readout" aria-live="polite">
          <span className="im-readout-label">En x = {fmt(x0)}</span>
          <span className="im-readout-value">
            pendiente <small>≈ {fmt(slope)}</small>
          </span>
          <span className="im-hint">{Math.abs(slope) < 1e-3 ? "tangente horizontal" : slope > 0 ? "la función sube ↗" : "la función baja ↘"}</span>
        </div>
      )}
      {area && (
        <div className="im-readout" aria-live="polite">
          <span className="im-readout-label">
            Área de {fmt(area.from)} a {fmt(upper)}
          </span>
          <span className="im-readout-value">≈ {fmt(exact)}</span>
          {sum && (
            <span className="im-hint">
              {rects} rectángulos suman {fmt(sum.total)} (diferencia {fmt(sum.total - exact)})
            </span>
          )}
          {accumulated.length > 1 && <span className="im-hint">La curva punteada es A(x): su pendiente en {fmt(upper)} vale f({fmt(upper)}) = {fmt(f(upper))}</span>}
        </div>
      )}
      <div className="im-sliders">
        {area && (
          <Slider
            v={{ sym: "b", label: "Hasta x", min: area.from, max: hi, init: upper, step: (hi - area.from) / 200 }}
            value={upper}
            color={COLORS[2]}
            disabled={locked}
            onChange={setUpper}
          />
        )}
        {sum && <Slider v={{ sym: "n", label: "Rectángulos", min: 1, max: 40, init: rects, step: 1 }} value={rects} color={COLORS[2]} disabled={locked} onChange={setRects} />}
        {tangent && (
          <Slider
            v={{ sym: "x0", label: "Punto x", min: lo, max: hi, init: x0, step: (hi - lo) / 200 }}
            value={x0}
            color={COLORS[2]}
            disabled={locked}
            onChange={setX0}
          />
        )}
        {spec.params.map((p, i) => (
          <Slider key={p.sym} v={p} value={vals[p.sym]} color={COLORS[i % COLORS.length]} disabled={locked} onChange={(n) => setVals((s) => ({ ...s, [p.sym]: n }))} />
        ))}
      </div>
      {challenge && chParam && <Challenge challenge={challenge} current={cur} range={chParam.max - chParam.min} locked={locked} />}
    </Frame>
  );
}

// --- vector_sum ------------------------------------------------------------

function VectorSum({ spec, locked }: { spec: Spec<"vector_sum">; locked: boolean }) {
  const [vecs, setVecs] = useState(() => spec.vectors.map((v) => ({ x: v.x, y: v.y })));
  const [tipToTail, setTipToTail] = useState(false);
  const R = vecs.reduce((a, v) => ({ x: a.x + v.x, y: a.y + v.y }), { x: 0, y: 0 });
  const S = 300;
  const U = S / 22; // grid units are -11..11
  const c = S / 2;
  const px = (x: number) => c + x * U;
  const py = (y: number) => c - y * U;
  const axis = { min: -10, max: 10, step: 0.5, init: 0 };

  let ox = 0;
  let oy = 0;
  const arrows = vecs.map((v, i) => {
    const from = tipToTail ? { x: ox, y: oy } : { x: 0, y: 0 };
    if (tipToTail) {
      ox += v.x;
      oy += v.y;
    }
    return { from, to: { x: from.x + v.x, y: from.y + v.y }, color: COLORS[i % COLORS.length], label: spec.vectors[i].label };
  });

  return (
    <Frame locked={locked}>
      <svg viewBox={`0 0 ${S} ${S}`} className="im-plot im-square" role="img" aria-label="Suma de vectores">
        <defs>
          {[...COLORS, RESULT].map((col, i) => (
            <marker key={i} id={`im-vec-${i}`} viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
              <path d="M0 0 L10 5 L0 10 z" fill={col} />
            </marker>
          ))}
        </defs>
        {Array.from({ length: 21 }, (_, i) => i - 10).map((g) => (
          <g key={g} stroke={NEUTRAL} strokeOpacity={g === 0 ? 0.9 : 0.18} strokeWidth="1">
            <line x1={px(g)} y1={py(-10)} x2={px(g)} y2={py(10)} />
            <line x1={px(-10)} y1={py(g)} x2={px(10)} y2={py(g)} />
          </g>
        ))}
        {arrows.map((a, i) => (
          <g key={i}>
            <line x1={px(a.from.x)} y1={py(a.from.y)} x2={px(a.to.x)} y2={py(a.to.y)} stroke={a.color} strokeWidth="2.5" markerEnd={`url(#im-vec-${i})`} />
            <text x={px((a.from.x + a.to.x) / 2) + 6} y={py((a.from.y + a.to.y) / 2) - 6} fill={a.color} fontSize="12" fontWeight="600" paintOrder="stroke" stroke="#121417" strokeWidth="3">
              {a.label}
            </text>
          </g>
        ))}
        {spec.showResultant && (
          <line x1={px(0)} y1={py(0)} x2={px(R.x)} y2={py(R.y)} stroke={RESULT} strokeWidth="3" strokeDasharray="7 4" markerEnd={`url(#im-vec-${COLORS.length})`} />
        )}
      </svg>
      {spec.showResultant && (
        <div className="im-readout" aria-live="polite">
          <span className="im-readout-label">Resultante</span>
          <span className="im-readout-value">
            ({fmt(R.x)}, {fmt(R.y)}) <small>|R| = {fmt(Math.hypot(R.x, R.y))}</small>
          </span>
        </div>
      )}
      <label className="im-check">
        <input type="checkbox" checked={tipToTail} onChange={(e) => setTipToTail(e.target.checked)} /> Ver punta a cola
      </label>
      <div className="im-sliders">
        {vecs.map((v, i) => (
          <div key={i} className="im-vec-pair">
            <Slider v={{ ...axis, sym: `x${i}`, label: `${spec.vectors[i].label} · x` }} value={v.x} color={COLORS[i % COLORS.length]} disabled={locked} onChange={(n) => setVecs((s) => s.map((q, j) => (j === i ? { ...q, x: n } : q)))} />
            <Slider v={{ ...axis, sym: `y${i}`, label: `${spec.vectors[i].label} · y` }} value={v.y} color={COLORS[i % COLORS.length]} disabled={locked} onChange={(n) => setVecs((s) => s.map((q, j) => (j === i ? { ...q, y: n } : q)))} />
          </div>
        ))}
      </div>
    </Frame>
  );
}

// --- particle_gradient -----------------------------------------------------

/** Small deterministic PRNG so the same spec always draws the same particles. */

function ParticleGradient({ spec, locked }: { spec: Spec<"particle_gradient">; locked: boolean }) {
  const [t, setT] = useState(0);
  const [playing, setPlaying] = useState(false);
  const raf = useRef<number | null>(null);

  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      let done = false;
      setT((p) => {
        const n = Math.min(1, p + dt / 6);
        done = n >= 1;
        return n;
      });
      if (done) setPlaying(false);
      else raf.current = requestAnimationFrame(tick);
    };
    raf.current = requestAnimationFrame(tick);
    return () => {
      if (raf.current !== null) cancelAnimationFrame(raf.current);
    };
  }, [playing]);

  const W = 360;
  const H = 200;
  const mid = W / 2;
  const barrier = spec.hasBarrier;
  const left = { x0: 12, x1: barrier ? mid - 8 : mid - 2 };
  const right = { x0: barrier ? mid + 8 : mid + 2, x1: W - 12 };

  const parts = useMemo(() => {
    const rnd = mulberry32(spec.highCount * 7919 + spec.lowCount * 104729 + 13);
    const place = (r: { x0: number; x1: number }) => ({ x: r.x0 + rnd() * (r.x1 - r.x0), y: 14 + rnd() * (H - 28) });
    const high = Array.from({ length: spec.highCount }, () => ({ home: place(left), dest: place(right) }));
    const low = Array.from({ length: spec.lowCount }, () => place(right));
    return { high, low };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spec.highCount, spec.lowCount, barrier]);

  const moved = Math.round(((spec.highCount - spec.lowCount) / 2) * t);
  const leftCount = spec.highCount - moved;
  const rightCount = spec.lowCount + moved;
  const channels = barrier ? Math.max(1, spec.channels) : 0;
  const gap = H / (channels + 1);
  const balanced = leftCount <= rightCount;

  return (
    <Frame locked={locked}>
      <svg viewBox={`0 0 ${W} ${H}`} className="im-plot" role="img" aria-label={`Partículas: ${spec.highLabel} y ${spec.lowLabel}`}>
        <rect x="2" y="2" width={mid - 4} height={H - 4} rx="8" fill={COLORS[0]} fillOpacity="0.07" />
        <rect x={mid + 2} y="2" width={mid - 4} height={H - 4} rx="8" fill={COLORS[1]} fillOpacity="0.07" />
        {barrier &&
          Array.from({ length: channels + 1 }, (_, i) => {
            const y0 = i === 0 ? 0 : i * gap + 11;
            const y1 = i === channels ? H : (i + 1) * gap - 11;
            return <rect key={i} x={mid - 6} y={y0} width="12" height={Math.max(0, y1 - y0)} rx="3" fill={NEUTRAL} fillOpacity="0.55" />;
          })}
        {!barrier && <line x1={mid} y1="8" x2={mid} y2={H - 8} stroke={NEUTRAL} strokeDasharray="4 4" />}
        {parts.high.map((p, i) => {
          const isMoved = i >= spec.highCount - moved;
          const at = isMoved ? p.dest : p.home;
          return <circle key={`h${i}`} cx={at.x} cy={at.y} r="5" fill={COLORS[0]} style={{ transition: "cx 0.45s ease, cy 0.45s ease" }} />;
        })}
        {parts.low.map((p, i) => (
          <circle key={`l${i}`} cx={p.x} cy={p.y} r="5" fill={COLORS[1]} />
        ))}
        <text x={mid / 2} y={H - 4} fill={NEUTRAL} fontSize="11" textAnchor="middle" paintOrder="stroke" stroke="#121417" strokeWidth="3">{spec.highLabel}</text>
        <text x={mid + mid / 2} y={H - 4} fill={NEUTRAL} fontSize="11" textAnchor="middle" paintOrder="stroke" stroke="#121417" strokeWidth="3">{spec.lowLabel}</text>
      </svg>
      <div className="im-readout" aria-live="polite">
        <span className="im-readout-label">Partículas</span>
        <span className="im-readout-value">
          {leftCount} <small>·</small> {rightCount}
        </span>
        <span className="im-hint">{balanced ? "equilibrio: pasa igual en ambos sentidos" : "más partículas a la izquierda: el flujo neto va hacia la derecha"}</span>
      </div>
      <div className="im-sliders">
        <Slider v={{ sym: "t", label: "Tiempo", min: 0, max: 1, step: 0.01, init: 0 }} value={t} disabled={locked} onChange={(n) => { setPlaying(false); setT(n); }} />
        <div className="row">
          <button type="button" className="btn-quiet" disabled={locked} onClick={() => { if (t >= 1) setT(0); setPlaying((p) => !p); }}>
            {playing ? "Pausar" : t >= 1 ? "Repetir" : "Reproducir"}
          </button>
        </div>
      </div>
    </Frame>
  );
}

// --- timeline --------------------------------------------------------------

function Timeline({ spec }: { spec: Spec<"timeline"> }) {
  const [i, setI] = useState(0);
  const last = spec.events.length - 1;
  return (
    <div className="im-frame">
      <ol className="im-timeline">
        {spec.events.map((e, idx) => (
          <li key={idx} className={idx === i ? "is-current" : idx < i ? "is-seen" : "is-future"}>
            <button type="button" onClick={() => setI(idx)} aria-current={idx === i ? "step" : undefined}>
              <span className="im-timeline-dot" aria-hidden="true" />
              <span className="im-timeline-label">{e.label}</span>
            </button>
            {idx === i && <p className="im-timeline-detail">{e.detail}</p>}
          </li>
        ))}
      </ol>
      <div className="row">
        <button type="button" className="btn-quiet" disabled={i === 0} onClick={() => setI((n) => Math.max(0, n - 1))}>Anterior</button>
        <button type="button" className="btn-primary" disabled={i === last} onClick={() => setI((n) => Math.min(last, n + 1))}>{i === last ? "Final" : "Siguiente"}</button>
      </div>
    </div>
  );
}

// --- entry -----------------------------------------------------------------

/** Renders one module. `locked` freezes the controls at their initial state
 *  (used by prediction gates: predict first, THEN move the sliders). */
export function InteractiveModuleVisual({
  spec,
  challenge,
  caption,
  locked = false,
}: {
  spec: InteractiveModuleSpec;
  challenge?: ModuleChallenge | null;
  caption?: string | null;
  locked?: boolean;
}) {
  let body: ReactNode;
  switch (spec.module) {
    case "relation_explorer":
      body = <RelationExplorer spec={spec} challenge={challenge} locked={locked} />;
      break;
    case "function_plot":
      body = <FunctionPlot spec={spec} challenge={challenge} locked={locked} />;
      break;
    case "vector_sum":
      body = <VectorSum spec={spec} locked={locked} />;
      break;
    case "particle_gradient":
      body = <ParticleGradient spec={spec} locked={locked} />;
      break;
    case "timeline":
      body = <Timeline spec={spec} />;
      break;
  }
  return (
    <figure className="static-visual interactive-module">
      {body}
      {caption && <figcaption className="hint">{caption}</figcaption>}
    </figure>
  );
}
