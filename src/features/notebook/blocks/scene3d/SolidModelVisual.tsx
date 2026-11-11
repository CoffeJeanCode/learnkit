import { useEffect, useMemo, useRef, useState } from "react";
import { CAMERA_TARGET, SOLID_VIEWS, type StaticVisualSpec } from "../../../../lib/schemas";
import { partColor } from "../../../../lib/scene3d/palette";
import { VIEW_LABEL, featureMeasures, isAdditive } from "../../../../lib/scene3d/solidBuild";
import type { SolidController } from "./solidStage";

type Spec = Extract<StaticVisualSpec, { renderEngine: "solid_model" }>;
type GlState = "pending" | "ready" | "unavailable";

const OP_VERB: Record<string, string> = { extrude: "Extrusión", revolve: "Revolución", cut: "Corte", hole: "Agujero" };

/** What a walkthrough step points at: a view, the camera, or an operation. */
export function resolveSolidTarget(features: Spec["features"], needle?: string): { view?: string; camera?: true; feature?: string } | null {
  const wanted = needle?.trim().toLowerCase();
  if (!wanted) return null;
  if ((SOLID_VIEWS as readonly string[]).includes(wanted)) return { view: wanted };
  if (wanted === CAMERA_TARGET) return { camera: true };
  const f = features.find((x) => x.id.toLowerCase() === wanted || x.label.trim().toLowerCase() === wanted);
  return f ? { feature: f.id } : null;
}

/** A CAD part with real measurements that the student orbits, rebuilds one
 *  operation at a time and sees from the standard views — and, with the
 *  camera inset, from outside: the camera moves, the part does not. The
 *  operation list under the figure is the keyboard/screen-reader path to the
 *  same content, and the whole visual when WebGL is not available. */
export function SolidModelVisual({ visual, highlightGroupId }: { visual: Spec; highlightGroupId?: string }) {
  const { features } = visual;
  const units = visual.units || "mm";
  const hostRef = useRef<HTMLDivElement>(null);
  const ctrlRef = useRef<SolidController | null>(null);
  const [gl, setGl] = useState<GlState>("pending");
  const [step, setStep] = useState(features.length);
  const [selected, setSelected] = useState<string | null>(null);
  const [view, setView] = useState<string>(visual.startView ?? "iso");
  // Each request moves the camera, even to the view already chosen (after orbiting away).
  const [viewRequest, setViewRequest] = useState(0);
  const [ortho, setOrtho] = useState(false);
  const [inset, setInset] = useState(!!visual.showCamera);
  const [labels, setLabels] = useState(true);

  const sceneKey = useMemo(() => JSON.stringify([features, visual.startView]), [features, visual.startView]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    import("./solidStage")
      .then(({ mountSolid }) => {
        if (cancelled) return;
        const ctrl = mountSolid(host, { features, startView: visual.startView ?? "iso" }, { onPick: (id) => setSelected((cur) => (id === cur ? null : id)) });
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

  // A guided-walkthrough step turns to a view, shows the camera, or focuses an operation.
  useEffect(() => {
    const target = resolveSolidTarget(features, highlightGroupId);
    if (!target) return;
    if (target.view) chooseView(target.view);
    if (target.camera) setInset(true);
    if (target.feature) focusFeature(target.feature);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [highlightGroupId, sceneKey]);

  useEffect(() => {
    if (gl !== "ready") return;
    const ctrl = ctrlRef.current;
    ctrl?.setStep(step);
    ctrl?.select(selected);
    ctrl?.setOrthographic(ortho);
    ctrl?.setCameraInset(inset);
    ctrl?.setLabels(labels);
  }, [gl, step, selected, ortho, inset, labels]);

  // Applied once the stage exists too: a walkthrough step may ask for a view before WebGL is up.
  useEffect(() => {
    if (gl === "ready" && viewRequest > 0) ctrlRef.current?.setView(view);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gl, viewRequest]);

  function chooseView(v: string) {
    setView(v);
    setViewRequest((n) => n + 1);
  }

  /** Selecting an operation also rebuilds the part far enough to show it. */
  function focusFeature(id: string) {
    const index = features.findIndex((f) => f.id === id);
    if (index < 0) return;
    setSelected(id);
    setStep((s) => Math.max(s, index + 1));
  }

  const toggle = (id: string) => (selected === id ? setSelected(null) : focusFeature(id));
  const current = features[step - 1];
  const selectedFeature = features.find((f) => f.id === selected);
  const has3d = gl !== "unavailable";

  return (
    <figure className="static-visual scene3d solid-model">
      {visual.title && (
        <div className="scene3d-bar">
          <strong className="scene3d-title">{visual.title}</strong>
        </div>
      )}
      <div
        ref={hostRef}
        className="scene3d-stage scene3d-stage--model"
        hidden={!has3d}
        role="img"
        aria-label={`Pieza en 3D con ${features.length} operaciones: ${features.map((f) => f.label).join(", ")}`}
      >
        {has3d && inset && (
          <div className="solid-inset" aria-hidden="true">
            <span>Tu cámara, vista desde fuera</span>
          </div>
        )}
      </div>
      {!has3d && <p className="hint">La vista 3D no está disponible en este equipo. Abajo tienes cada operación explicada.</p>}

      {has3d && (
        <div className="scene3d-controls">
          <div className="scene3d-tabs" role="group" aria-label="Vistas estándar">
            {SOLID_VIEWS.map((v) => (
              <button key={v} type="button" className={v === view ? "is-current" : undefined} aria-pressed={v === view} onClick={() => chooseView(v)}>
                {VIEW_LABEL[v]}
              </button>
            ))}
          </div>
          <button type="button" className="btn-quiet" aria-pressed={ortho} onClick={() => setOrtho((o) => !o)}>
            {ortho ? "Perspectiva" : "Ortográfica"}
          </button>
          <button type="button" className="btn-quiet" aria-pressed={inset} onClick={() => setInset((v) => !v)}>
            {inset ? "Ocultar tu cámara" : "Ver tu cámara"}
          </button>
          <button type="button" className="btn-quiet" aria-pressed={labels} onClick={() => setLabels((v) => !v)}>
            {labels ? "Ocultar etiquetas" : "Mostrar etiquetas"}
          </button>
        </div>
      )}

      {features.length > 1 && (
        <div className="scene3d-step solid-steps">
          <div className="solid-steps-bar">
            <button type="button" className="btn-quiet" disabled={step <= 1} onClick={() => setStep((s) => s - 1)} aria-label="Operación anterior">
              ←
            </button>
            <span className="solid-steps-count">
              Operación {step} de {features.length}
            </span>
            <button type="button" className="btn-quiet" disabled={step >= features.length} onClick={() => setStep((s) => s + 1)} aria-label="Operación siguiente">
              →
            </button>
          </div>
          {current && (
            <p>
              <strong>
                {OP_VERB[current.op] ?? current.op}: {current.label}
              </strong>{" "}
              <span className="hint">({featureMeasures(current, units)})</span>
            </p>
          )}
        </div>
      )}

      <ul className="scene3d-legend" aria-label="Operaciones de la pieza">
        {features.map((f, i) => (
          <li key={f.id}>
            <button
              type="button"
              className={`scene3d-chip${f.id === selected ? " is-current" : ""}${i >= step ? " is-future" : ""}`}
              aria-pressed={f.id === selected}
              onClick={() => toggle(f.id)}
            >
              <span className="scene3d-dot" style={{ background: isAdditive(f.op) ? partColor(null, i) : partColor("rose", i) }} />
              {f.label}
            </button>
          </li>
        ))}
      </ul>

      {selectedFeature ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>
            {OP_VERB[selectedFeature.op] ?? selectedFeature.op}: {selectedFeature.label}
          </strong>
          <p className="hint">{featureMeasures(selectedFeature, units)}</p>
          <p>{selectedFeature.explanation}</p>
          <button type="button" className="btn-quiet" onClick={() => setSelected(null)}>
            Cerrar
          </button>
        </aside>
      ) : (
        has3d && <p className="hint node-note-hint">Arrastra para orbitar: se mueve tu cámara, la pieza y sus ejes se quedan quietos. Toca una cara para ver qué operación la hizo.</p>
      )}

      <details className="walkthrough-all" open={!has3d}>
        <summary>Ver todas las operaciones</summary>
        <ol className="walkthrough-list">
          {features.map((f) => (
            <li key={f.id}>
              <strong>
                {OP_VERB[f.op] ?? f.op} — {f.label}
              </strong>{" "}
              ({featureMeasures(f, units)}): {f.explanation}
            </li>
          ))}
        </ol>
      </details>
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
