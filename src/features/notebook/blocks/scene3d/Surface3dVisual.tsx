import { useEffect, useMemo, useRef, useState } from "react";
import type { StaticVisualSpec } from "../../../../lib/schemas";
import { partColor } from "../../../../lib/scene3d/palette";
import { compileSurface, displayScale, sampleSurface, tracePaths, volumeUnder } from "../../../../lib/scene3d/surfaceBuild";
import { Slider } from "../InteractiveModule";
import type { SurfaceController } from "./surfaceStage";

type Spec = Extract<StaticVisualSpec, { renderEngine: "surface_3d" }>;
type GlState = "pending" | "ready" | "unavailable";

const KIND_LABEL: Record<string, string> = { geodesic: "Geodésica", descent: "Descenso", level: "Curva de nivel" };

/** The id a walkthrough step points at (a path or a marker), by id or label. */
export function resolveSurfaceTarget(spec: Pick<Spec, "paths" | "markers">, needle?: string): string | null {
  const wanted = needle?.trim().toLowerCase();
  if (!wanted) return null;
  const all = [...spec.paths, ...spec.markers];
  return all.find((x) => x.id.toLowerCase() === wanted || x.label.trim().toLowerCase() === wanted)?.id ?? null;
}

const fmt = (n: number) => (Math.abs(n) >= 100 ? n.toFixed(0) : Math.abs(n) >= 1 ? n.toFixed(2) : n.toPrecision(3));

/** A surface z = f(x, y) the student turns around and reshapes with sliders;
 *  every path on it is traced by the system (real geodesics, descents, level
 *  curves), so what bends is what the math says bends. The legend and the
 *  list under the figure carry the same content without WebGL. */
export function Surface3dVisual({ visual, highlightGroupId }: { visual: Spec; highlightGroupId?: string }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const ctrlRef = useRef<SurfaceController | null>(null);
  const [gl, setGl] = useState<GlState>("pending");
  const [values, setValues] = useState<Record<string, number>>(() => Object.fromEntries(visual.params.map((p) => [p.sym, p.init])));
  const [selected, setSelected] = useState<string | null>(null);
  const [animate, setAnimate] = useState(true);
  const [labels, setLabels] = useState(true);
  const xr = visual.xRange;
  const yr = visual.yRange;
  const bounds = useMemo(() => ({ xr, yr }), [xr, yr]);

  // Exaggeration fixed at the INITIAL values: a heavier mass then visibly digs deeper.
  const scale = useMemo(() => {
    const f0 = compileSurface(visual.expr, Object.fromEntries(visual.params.map((p) => [p.sym, p.init])));
    if (!f0) return 1;
    const g = sampleSurface(f0, xr, yr, 33);
    return displayScale(xr, yr, g.zMin, g.zMax);
  }, [visual.expr, visual.params, xr, yr]);

  const colors = useMemo(() => {
    const ids = [...visual.paths.map((p) => p.id), ...visual.markers.map((m) => m.id)];
    return Object.fromEntries(ids.map((id, i) => [id, partColor(null, i)]));
  }, [visual.paths, visual.markers]);

  const computed = useMemo(() => {
    const f = compileSurface(visual.expr, values);
    if (!f) return null;
    const grid = sampleSurface(f, xr, yr, 64);
    const traced = tracePaths(f, scale, bounds, visual.paths);
    const volume = visual.region ? volumeUnder(f, visual.region) : null;
    return { f, grid, traced, volume };
  }, [visual.expr, visual.paths, visual.region, values, xr, yr, scale, bounds]);

  const sceneKey = useMemo(() => JSON.stringify([visual.expr, xr, yr]), [visual.expr, xr, yr]);
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    import("./surfaceStage")
      .then(({ mountSurface }) => {
        if (cancelled) return;
        const ctrl = mountSurface(host, xr, yr, { onPick: (id) => setSelected((cur) => (cur === id ? null : id)) });
        if (!ctrl) {
          setGl("unavailable");
          return;
        }
        ctrlRef.current = ctrl;
        setGl("ready");
      })
      .catch(() => {
        if (!cancelled) setGl("unavailable");
      });
    return () => {
      cancelled = true;
      ctrlRef.current?.dispose();
      ctrlRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sceneKey]);

  useEffect(() => {
    if (gl !== "ready" || !computed) return;
    ctrlRef.current?.setScene({
      grid: computed.grid,
      scale,
      paths: computed.traced.map((t) => ({ ...t, label: visual.paths.find((p) => p.id === t.id)?.label ?? t.id, color: colors[t.id] })),
      markers: visual.markers.map((m) => ({ id: m.id, label: m.label, color: colors[m.id], at: [m.at[0], m.at[1], scale * (computed.f(m.at[0], m.at[1]) || 0)] })),
      region: visual.region,
      zLabel: visual.zLabel,
    });
  }, [gl, computed, scale, colors, visual.paths, visual.markers, visual.region, visual.zLabel]);

  useEffect(() => {
    if (gl !== "ready") return;
    ctrlRef.current?.select(selected);
    ctrlRef.current?.setAnimate(animate);
    ctrlRef.current?.setLabels(labels);
  }, [gl, selected, animate, labels]);

  useEffect(() => {
    const id = resolveSurfaceTarget(visual, highlightGroupId);
    if (id) setSelected(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [highlightGroupId, sceneKey]);

  const items = [
    ...visual.paths.map((p) => ({ id: p.id, label: p.label, kind: KIND_LABEL[p.kind] ?? p.kind, explanation: p.explanation })),
    ...visual.markers.map((m) => ({ id: m.id, label: m.label, kind: "Punto", explanation: m.explanation })),
  ];
  const current = items.find((i) => i.id === selected);
  const has3d = gl !== "unavailable";

  return (
    <figure className="static-visual scene3d surface-3d">
      {visual.title && (
        <div className="scene3d-bar">
          <strong className="scene3d-title">{visual.title}</strong>
        </div>
      )}
      <div ref={hostRef} className="scene3d-stage scene3d-stage--model" hidden={!has3d} role="img" aria-label={`Superficie z = ${visual.expr} con ${visual.paths.length} trayectorias`} />
      {!has3d && <p className="hint">La vista 3D no está disponible en este equipo. Abajo tienes cada trayectoria explicada.</p>}
      {!computed && <p className="hint hint-warn">No se pudo calcular esta superficie.</p>}

      {visual.params.length > 0 && (
        <div className="im-sliders">
          {visual.params.map((p, i) => (
            <Slider key={p.sym} v={p} value={values[p.sym]} color={partColor(null, i + 2)} disabled={false} onChange={(n) => setValues((s) => ({ ...s, [p.sym]: n }))} />
          ))}
        </div>
      )}
      {computed?.volume != null && (
        <div className="im-readout" aria-live="polite">
          <span className="im-readout-label">Volumen bajo la superficie en la región</span>
          <span className="im-readout-value">≈ {fmt(computed.volume)}</span>
        </div>
      )}

      {has3d && (
        <div className="scene3d-controls">
          {visual.paths.length > 0 && (
            <button type="button" className="btn-quiet" aria-pressed={animate} onClick={() => setAnimate((v) => !v)}>
              {animate ? "Pausar recorrido" : "Animar recorrido"}
            </button>
          )}
          <button type="button" className="btn-quiet" aria-pressed={labels} onClick={() => setLabels((v) => !v)}>
            {labels ? "Ocultar etiquetas" : "Mostrar etiquetas"}
          </button>
        </div>
      )}

      {items.length > 0 && (
        <ul className="scene3d-legend" aria-label="Trayectorias y puntos">
          {items.map((it) => (
            <li key={it.id}>
              <button type="button" className={`scene3d-chip${it.id === selected ? " is-current" : ""}`} aria-pressed={it.id === selected} onClick={() => setSelected((c) => (c === it.id ? null : it.id))}>
                <span className="scene3d-dot" style={{ background: colors[it.id] }} />
                {it.label}
              </button>
            </li>
          ))}
        </ul>
      )}

      {current ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>
            {current.kind}: {current.label}
          </strong>
          <p>{current.explanation}</p>
          <button type="button" className="btn-quiet" onClick={() => setSelected(null)}>
            Cerrar
          </button>
        </aside>
      ) : (
        has3d && <p className="hint node-note-hint">Arrastra para girar la superficie. Las trayectorias las calcula el sistema sobre la forma real: toca una para ver qué muestra.</p>
      )}

      {items.length > 0 && (
        <details className="walkthrough-all" open={!has3d}>
          <summary>Ver todas las trayectorias</summary>
          <ol className="walkthrough-list">
            {items.map((it) => (
              <li key={it.id}>
                <strong>
                  {it.kind} — {it.label}:
                </strong>{" "}
                {it.explanation}
              </li>
            ))}
          </ol>
        </details>
      )}
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
