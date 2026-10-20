import { useState } from "react";
import type { DynamicSectionBlock } from "../../../lib/schemas";
import { InlineText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "declarative_visual_diagram" }>;

/** Walks the figure one step at a time: the active step is explained right
 *  under the diagram while its SVG group (`data-group-id` ==
 *  `targetVisualElement`) is highlighted, so the explanation and the thing it
 *  explains stay together (Mayer's contiguity principle) instead of a list
 *  the eye has to cross-reference with the figure. */
export function DeclarativeVisualDiagramBlock({ content }: { content: Content }) {
  const steps = content.guidedWalkthrough.slice().sort((a, b) => a.stepNumber - b.stepNumber);
  const [index, setIndex] = useState(0);
  const step = steps[Math.min(index, steps.length - 1)];

  return (
    <div className="notebook-block-body">
      <h3>
        <InlineText text={content.title} />
      </h3>
      <StaticVisual visual={content.visualAid} highlightGroupId={step?.targetVisualElement} />
      {step && (
        <div className="walkthrough-stepper" aria-live="polite">
          <div className="walkthrough-progress">
            <span className="badge">
              Paso {index + 1} de {steps.length}
            </span>
            <div className="walkthrough-dots" aria-hidden="true">
              {steps.map((s, i) => (
                <span key={s.stepNumber} className={`walkthrough-dot${i === index ? " is-current" : i < index ? " is-seen" : ""}`} />
              ))}
            </div>
          </div>
          <p className="walkthrough-step">
            <strong>
              <InlineText text={step.targetVisualElement} />:
            </strong>{" "}
            <InlineText text={step.pedagogicalInsight} />
          </p>
          <div className="row">
            <button className="btn-quiet" onClick={() => setIndex((i) => Math.max(0, i - 1))} disabled={index === 0}>
              Paso anterior
            </button>
            <button className="btn-primary" onClick={() => setIndex((i) => Math.min(steps.length - 1, i + 1))} disabled={index >= steps.length - 1}>
              {index >= steps.length - 1 ? "Recorrido completo" : "Siguiente paso"}
            </button>
          </div>
          <details className="walkthrough-all">
            <summary>Ver todos los pasos</summary>
            <ol className="walkthrough-list">
              {steps.map((s) => (
                <li key={s.stepNumber}>
                  <strong>
                    <InlineText text={s.targetVisualElement} />:
                  </strong>{" "}
                  <InlineText text={s.pedagogicalInsight} />
                </li>
              ))}
            </ol>
          </details>
        </div>
      )}
    </div>
  );
}
