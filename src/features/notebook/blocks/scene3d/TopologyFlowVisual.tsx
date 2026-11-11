import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { StaticVisualSpec } from "../../../../lib/schemas";
import { TONE_LABEL, toneColor } from "../../../../lib/scene3d/palette";
import { topologyToSequence } from "../../../../lib/scene3d/topologyLayout";
import type { TopologyController } from "./topologyStage";

type Spec = Extract<StaticVisualSpec, { renderEngine: "topology_flow" }>;

/** `pending` until the 3D bundle has loaded and a WebGL context exists. */
type GlState = "pending" | "ready" | "unavailable";
/** How the current step is showing: its final picture, mid-animation, or
 *  animation completed (which is what lets playback move on). */
type Phase = "static" | "running" | "done";

const SPEEDS = [0.5, 1, 2];

/** Finds the node a walkthrough step points at, by id or by visible label. */
function resolveNode(nodes: Spec["nodes"], needle?: string): string | null {
  const wanted = needle?.trim().toLowerCase();
  if (!wanted) return null;
  return nodes.find((n) => n.id.toLowerCase() === wanted || n.label.trim().toLowerCase() === wanted)?.id ?? null;
}

/** An isometric topology whose message flow the student plays step by step
 *  ("Red"), with the same story as a sequence diagram one tab away
 *  ("Secuencia"). The model wrote only nodes, links and steps — layout,
 *  routing and animation are ours (`lib/scene3d/topologyLayout.ts`,
 *  `topologyStage.ts`). Without WebGL the sequence diagram is the visual. */
export function TopologyFlowVisual({
  visual,
  highlightGroupId,
  renderSequence,
}: {
  visual: Spec;
  highlightGroupId?: string;
  /** Draws Mermaid source (injected by `StaticVisual`, which owns Mermaid). */
  renderSequence: (code: string) => ReactNode;
}) {
  const { nodes, links, steps } = visual;
  const hostRef = useRef<HTMLDivElement>(null);
  const ctrlRef = useRef<TopologyController | null>(null);
  const advanceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [gl, setGl] = useState<GlState>("pending");
  const [view, setView] = useState<"red" | "secuencia">("red");
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [phase, setPhase] = useState<Phase>("static");
  const [runId, setRunId] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [openNode, setOpenNode] = useState<string | null>(null);

  // The stage callbacks outlive renders; they read the latest values from refs.
  const live = useRef({ step, playing, speed, last: steps.length - 1 });
  live.current = { step, playing, speed, last: steps.length - 1 };

  // Rebuild the scene only when the drawn content changes, not on every render.
  const sceneKey = useMemo(() => JSON.stringify([nodes, links, steps]), [nodes, links, steps]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    import("./topologyStage")
      .then(({ mountTopology }) => {
        if (cancelled) return;
        const ctrl = mountTopology(
          host,
          { nodes, links, steps },
          {
            onPickNode: (id) => setOpenNode((cur) => (cur === id ? null : id)),
            onStepFinished: (index) => {
              if (index !== live.current.step) return;
              setPhase("done");
              if (!live.current.playing) return;
              if (index >= live.current.last) {
                setPlaying(false);
                return;
              }
              // A beat to read the step before the next one starts.
              advanceTimer.current = setTimeout(() => setStep(index + 1), 900 / live.current.speed);
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
      if (advanceTimer.current) clearTimeout(advanceTimer.current);
      ctrlRef.current?.dispose();
      ctrlRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sceneKey]);

  // Show the current step: animated while playing, its final picture otherwise.
  useEffect(() => {
    if (advanceTimer.current) clearTimeout(advanceTimer.current);
    if (gl !== "ready") return;
    const animate = live.current.playing;
    setPhase(animate ? "running" : "static");
    ctrlRef.current?.playStep(step, animate);
  }, [gl, step, runId]);

  useEffect(() => {
    if (gl === "ready") ctrlRef.current?.setPaused(!playing);
  }, [gl, playing]);

  useEffect(() => {
    if (gl === "ready") ctrlRef.current?.setSpeed(speed);
  }, [gl, speed]);

  useEffect(() => {
    if (gl === "ready") ctrlRef.current?.setHighlight(resolveNode(nodes, highlightGroupId));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gl, highlightGroupId, sceneKey]);

  const lastStep = steps.length - 1;
  const current = steps[Math.min(step, lastStep)];
  const shownView = gl === "unavailable" ? "secuencia" : view;
  const messagesBefore = steps.slice(0, step).reduce((n, s) => n + s.messages.length, 0);
  const nodeLabel = (id: string) => nodes.find((n) => n.id === id)?.label ?? id;
  const note = openNode ? nodes.find((n) => n.id === openNode) : undefined;

  const goTo = (index: number) => {
    setPlaying(false);
    setStep(Math.max(0, Math.min(lastStep, index)));
  };

  const togglePlay = () => {
    if (playing) {
      setPlaying(false);
      return;
    }
    setPlaying(true);
    if (phase === "running") return; // paused mid-flight: just resume
    if (phase === "done") setStep(step >= lastStep ? 0 : step + 1);
    setRunId((n) => n + 1); // replay (or start) the step that is showing
  };

  return (
    <figure className="static-visual scene3d">
      <div className="scene3d-bar">
        {gl !== "unavailable" && (
          <div className="scene3d-tabs" role="tablist" aria-label="Vista del diagrama">
            <button type="button" role="tab" aria-selected={shownView === "red"} className={shownView === "red" ? "is-current" : ""} onClick={() => setView("red")}>
              Red
            </button>
            <button type="button" role="tab" aria-selected={shownView === "secuencia"} className={shownView === "secuencia" ? "is-current" : ""} onClick={() => setView("secuencia")}>
              Secuencia
            </button>
          </div>
        )}
        {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
        {steps.length > 0 && (
          <span className="badge">
            Paso {Math.min(step, lastStep) + 1} de {steps.length}
          </span>
        )}
      </div>

      <div
        ref={hostRef}
        className="scene3d-stage scene3d-stage--topology"
        hidden={shownView !== "red"}
        role="img"
        aria-label={`Diagrama de red: ${nodes.map((n) => n.label).join(", ")}`}
      />
      {shownView === "secuencia" && <div className="scene3d-sequence">{renderSequence(topologyToSequence(nodes, steps, step + 1))}</div>}

      {steps.length > 0 && (
        <div className="scene3d-controls">
          <button type="button" className="btn-quiet" onClick={() => goTo(step - 1)} disabled={step === 0}>
            Paso anterior
          </button>
          {gl !== "unavailable" && shownView === "red" && (
            <button type="button" className="btn-primary" onClick={togglePlay} aria-pressed={playing}>
              {playing ? "Pausa" : phase === "done" && step >= lastStep ? "Repetir" : "Reproducir"}
            </button>
          )}
          <button type="button" className="btn-quiet" onClick={() => goTo(step + 1)} disabled={step >= lastStep}>
            Siguiente paso
          </button>
          {gl !== "unavailable" && shownView === "red" && (
            <label className="scene3d-speed">
              Velocidad
              <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))}>
                {SPEEDS.map((s) => (
                  <option key={s} value={s}>
                    {s}×
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}

      {current && (
        <div className="scene3d-step" aria-live="polite">
          <strong>{current.title}</strong>
          {current.detail && <p>{current.detail}</p>}
          {current.messages.length > 0 && (
            <table className="scene3d-messages">
              <thead>
                <tr>
                  <th scope="col">N.º</th>
                  <th scope="col">Origen</th>
                  <th scope="col">Destino</th>
                  <th scope="col">Mensaje</th>
                </tr>
              </thead>
              <tbody>
                {current.messages.map((m, i) => (
                  <tr key={i}>
                    <td>{messagesBefore + i + 1}</td>
                    <td>{nodeLabel(m.from)}</td>
                    <td>{nodeLabel(m.to)}</td>
                    <td>
                      <span className="scene3d-dot" style={{ background: toneColor(m.tone) }} title={TONE_LABEL[m.tone ?? "request"]} />
                      {m.label}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {note?.explanation?.trim() && (
        <aside className="node-note" role="note">
          <strong>{note.label}</strong>
          <p>{note.explanation}</p>
          <button type="button" className="btn-quiet" onClick={() => setOpenNode(null)}>
            Cerrar
          </button>
        </aside>
      )}
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
