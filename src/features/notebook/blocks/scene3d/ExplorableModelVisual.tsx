import { useEffect, useMemo, useRef, useState } from "react";
import { SHELL_TARGET, type StaticVisualSpec } from "../../../../lib/schemas";
import { partColor } from "../../../../lib/scene3d/palette";
import { clampCount, isLayer } from "../../../../lib/scene3d/partPlacement";
import type { ModelController } from "./modelStage";

type Spec = Extract<StaticVisualSpec, { renderEngine: "explorable_model" }>;
type GlState = "pending" | "ready" | "unavailable";

/** Finds the part a walkthrough step points at, by id or by visible label. */
function resolvePart(parts: Spec["parts"], needle?: string, shell?: Spec["shell"]): string | null {
  const wanted = needle?.trim().toLowerCase();
  if (!wanted) return null;
  if (shell && (wanted === SHELL_TARGET || wanted === shell.label.trim().toLowerCase())) return SHELL_TARGET;
  return parts.find((p) => p.id.toLowerCase() === wanted || p.label.trim().toLowerCase() === wanted)?.id ?? null;
}

/** A schematic 3D object the student turns around, cuts open and taps part by
 *  part. The model chose shapes, sizes and zones from closed catalogues; where
 *  each part sits is ours (`lib/scene3d/partPlacement.ts`). The legend under
 *  the figure is a full keyboard/screen-reader path to the same content, and
 *  the whole visual when WebGL is not available. */
export function ExplorableModelVisual({ visual, highlightGroupId }: { visual: Spec; highlightGroupId?: string }) {
  const { parts, shell } = visual;
  const hostRef = useRef<HTMLDivElement>(null);
  const ctrlRef = useRef<ModelController | null>(null);
  const [gl, setGl] = useState<GlState>("pending");
  const [selected, setSelected] = useState<string | null>(null);
  const [isolated, setIsolated] = useState(false);
  const hasLayers = parts.some(isLayer);
  // Same default as the stage: a layered model only reads cut open.
  const [cut, setCut] = useState((shell?.cutaway ?? false) || hasLayers);
  const [labels, setLabels] = useState(true);

  const sceneKey = useMemo(() => JSON.stringify([parts, shell]), [parts, shell]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    import("./modelStage")
      .then(({ mountModel }) => {
        if (cancelled) return;
        const ctrl = mountModel(
          host,
          { shell, parts },
          {
            onPick: (id) => {
              setSelected((cur) => (id === cur ? null : id));
              if (!id) setIsolated(false);
            },
          },
        );
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

  // A guided-walkthrough step selects the part it talks about.
  useEffect(() => {
    const id = resolvePart(parts, highlightGroupId, shell);
    if (id) setSelected(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [highlightGroupId, sceneKey]);

  useEffect(() => {
    if (gl !== "ready") return;
    const ctrl = ctrlRef.current;
    ctrl?.select(selected);
    ctrl?.setIsolated(isolated);
    ctrl?.setCut(cut);
    ctrl?.setLabels(labels);
  }, [gl, selected, isolated, cut, labels]);

  const selectedIndex = parts.findIndex((p) => p.id === selected);
  const selectedPart = selectedIndex >= 0 ? parts[selectedIndex] : undefined;
  const has3d = gl !== "unavailable";

  const toggle = (id: string) => {
    setSelected((cur) => (cur === id ? null : id));
    if (selected === id) setIsolated(false);
  };

  return (
    <figure className="static-visual scene3d">
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
        aria-label={`Modelo 3D con ${parts.length} partes: ${parts.map((p) => p.label).join(", ")}`}
      />
      {!has3d && <p className="hint">La vista 3D no está disponible en este equipo. Abajo tienes cada parte explicada.</p>}

      {has3d && (
        <div className="scene3d-controls">
          <button type="button" className="btn-quiet" aria-pressed={isolated} disabled={!selectedPart} onClick={() => setIsolated((v) => !v)}>
            {isolated ? "Mostrar todo" : "Aislar parte"}
          </button>
          {(shell || hasLayers) && (
            <button type="button" className="btn-quiet" aria-pressed={cut} onClick={() => setCut((v) => !v)}>
              {cut ? "Cerrar corte" : "Ver corte"}
            </button>
          )}
          <button type="button" className="btn-quiet" aria-pressed={labels} onClick={() => setLabels((v) => !v)}>
            {labels ? "Ocultar etiquetas" : "Mostrar etiquetas"}
          </button>
          <button type="button" className="btn-quiet" onClick={() => ctrlRef.current?.resetView()}>
            Restablecer vista
          </button>
        </div>
      )}

      <ul className="scene3d-legend" aria-label="Partes del modelo">
        {parts.map((p, i) => (
          <li key={p.id}>
            <button type="button" className={`scene3d-chip${p.id === selected ? " is-current" : ""}`} aria-pressed={p.id === selected} onClick={() => toggle(p.id)}>
              <span className="scene3d-dot" style={{ background: partColor(p.color, i) }} />
              {p.label}
              {!isLayer(p) && clampCount(p.count) > 1 && <span className="scene3d-count">×{clampCount(p.count)}</span>}
            </button>
          </li>
        ))}
        {shell && (
          <li>
            <button
              type="button"
              className={`scene3d-chip${selected === SHELL_TARGET ? " is-current" : ""}`}
              aria-pressed={selected === SHELL_TARGET}
              onClick={() => toggle(SHELL_TARGET)}
            >
              <span className="scene3d-dot scene3d-dot--shell" />
              {shell.label}
            </button>
          </li>
        )}
      </ul>

      {selectedPart ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>{selectedPart.label}</strong>
          <p>{selectedPart.explanation}</p>
          <button type="button" className="btn-quiet" onClick={() => toggle(selectedPart.id)}>
            Cerrar
          </button>
        </aside>
      ) : selected === SHELL_TARGET && shell ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>{shell.label}</strong>
          <p>La envoltura que contiene todas las partes.</p>
          <button type="button" className="btn-quiet" onClick={() => toggle(SHELL_TARGET)}>
            Cerrar
          </button>
        </aside>
      ) : (
        has3d && <p className="hint node-note-hint">Arrastra para girar, usa la rueda para acercar y toca una parte para ver qué hace.</p>
      )}

      <details className="walkthrough-all" open={!has3d}>
        <summary>Ver todas las partes</summary>
        <ol className="walkthrough-list">
          {parts.map((p) => (
            <li key={p.id}>
              <strong>{p.label}:</strong> {p.explanation}
            </li>
          ))}
        </ol>
      </details>
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
