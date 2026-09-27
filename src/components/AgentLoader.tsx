import { useEffect, useState } from "react";

// Hand-drawn loaders in the sketch language: SVG strokes that draw and
// erase themselves in a loop (see `.sk-draw` in styles.css).

/** Small inline squiggle that sketches itself — the "thinking" mark. */
export function SketchingMark() {
  return (
    <svg className="sk-mark" viewBox="0 0 68 22" aria-hidden="true">
      <path d="M2 12 Q 10 2 18 12 T 34 12 T 50 12 T 66 12" className="sk-draw" />
    </svg>
  );
}

/** "Pensando…" row: a lone hand-drawn spiral that keeps redrawing itself,
 *  boiling (stepped jitter) and swaying like a pencil doodle in motion. */
export function ThinkingSketch({ label }: { label: string }) {
  return (
    <span className="thinking-sketch">
      <svg className="think-doodle" viewBox="6 10 50 46" aria-hidden="true">
        <g className="td-bob">
          <g className="td-boil">
            <path
              className="td-line td-spiral"
              pathLength={1}
              d="M32 30 C32 26 28 25 26 28 C24 31 27 35 32 35 C38 35 40 29 36 24 C31 18 22 18 18 25 C13 33 17 44 27 46 C39 48 48 39 46 28"
            />
          </g>
        </g>
      </svg>
      <span className="td-label">{label}</span>
    </span>
  );
}

/** "Trabajando…" row: two hand-drawn gears grinding — shown while the
 *  agent is RUNNING TOOLS (agent://tool), not just thinking its reply. */
export function ToolSketch({ label }: { label: string }) {
  return (
    <span className="thinking-sketch">
      <svg className="tool-doodle" viewBox="10 8 60 40" aria-hidden="true">
        <g className="td-bob">
          <g className="td-boil">
            <g transform="translate(28 30)">
              <g className="tl-gear tl-cw">
                <circle className="td-line" cx="0" cy="0" r="10" />
                <path
                  className="td-line"
                  d="M10 0 L13.5 0 M7.07 7.07 L9.55 9.55 M0 10 L0 13.5 M-7.07 7.07 L-9.55 9.55 M-10 0 L-13.5 0 M-7.07 -7.07 L-9.55 -9.55 M0 -10 L0 -13.5 M7.07 -7.07 L9.55 -9.55"
                />
              </g>
            </g>
            <g transform="translate(54 22)">
              <g className="tl-gear tl-ccw">
                <circle className="td-line" cx="0" cy="0" r="7" />
                <path
                  className="td-line"
                  d="M7 0 L10 0 M4.95 4.95 L7.07 7.07 M0 7 L0 10 M-4.95 4.95 L-7.07 7.07 M-7 0 L-10 0 M-4.95 -4.95 L-7.07 -7.07 M0 -7 L0 -10 M4.95 -4.95 L7.07 -7.07"
                />
              </g>
            </g>
          </g>
        </g>
      </svg>
      <span className="td-label">{label}</span>
    </span>
  );
}

// --- Class-generation loader -------------------------------------------------

export type GenerationStage = "preparando" | "escribiendo" | "revisando" | "reintentando";

export const GENERATION_STAGE_TEXT: Record<GenerationStage, string> = {
  preparando: "Abriendo tu plan y afilando el lápiz…",
  escribiendo: "El agente está escribiendo tu clase…",
  revisando: "Revisando que todo quede en su punto…",
  reintentando: "Salió chueco — borrando y reintentando…",
};

const GENERATION_TIPS = [
  "Tu primera clase se genera una sola vez y queda guardada en este equipo.",
  "El agente diseña teoría, ejemplos y práctica a tu medida.",
  "Cuando termine, podrás editar cada bloque a tu gusto.",
  "Si algo falla, reintenta solo — nada se rompe.",
];

/** Illustrative loader for notebook generation, driven by the real agent
 *  stage (see `useClassNotebookSession`'s `agent://` listeners) plus rotating
 *  tips so a long generation never looks stuck. */
export function ClassGenerationLoader({ stage }: { stage: GenerationStage }) {
  const [tipIndex, setTipIndex] = useState(0);

  useEffect(() => {
    const id = setInterval(() => setTipIndex((i) => (i + 1) % GENERATION_TIPS.length), 5000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="gen-loader">
      <svg className="gen-sketch" viewBox="0 0 140 100" aria-hidden="true">
        <rect x="30" y="6" width="80" height="88" className="sk-draw d1" />
        <line x1="42" y1="6" x2="42" y2="94" className="sk-draw sk-accent d2" />
        <line x1="54" y1="28" x2="98" y2="28" className="sk-draw d2" />
        <line x1="54" y1="44" x2="98" y2="44" className="sk-draw d3" />
        <line x1="54" y1="60" x2="82" y2="60" className="sk-draw d4" />
        <line x1="54" y1="76" x2="92" y2="76" className="sk-draw d5" />
      </svg>
      <div className="gen-text">
        <div className="gen-stage">{GENERATION_STAGE_TEXT[stage]}</div>
        <div className="gen-tip">✎ {GENERATION_TIPS[tipIndex]}</div>
      </div>
    </div>
  );
}
