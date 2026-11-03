import { useEffect, useRef, useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import { ConnectStep } from "../../graph/ConnectStep";
import { GateProgress, GateSubmitButton } from "./GateSubmitButton";
import { InlineText, RichText } from "./RichText";
import {
  IconCheck,
  IconLightbulb,
  IconPencil,
  IconSparkles,
  IconTarget,
} from "../../../components/Icons";

type Content = Extract<DynamicSectionBlock, { blockType: "metacognitive_closure" }>;

const SAVE_DEBOUNCE_MS = 800;

/** The class's final block: free-text reflection graded server-side
 *  (`gradeClosureReflection`). The student requests feedback explicitly; the
 *  verdict is ALWAYS student-facing — a fail says what to add and leaves the
 *  button available, a pass unlocks the continue CTA (which is also the
 *  moment `refreshClasses` can flip this class to complete and unlock the
 *  next one). An approved closure is frozen: reflection read-only, no more
 *  re-grading (backend refuses too). */
export function MetacognitiveClosureBlock({
  block,
  content,
  isActive,
  onSave,
  onRequestFeedback,
  onContinue,
  continueLabel,
}: {
  block: NotebookBlock;
  content: Content;
  isActive: boolean;
  onSave: (blockId: string, content: Record<string, unknown>) => void;
  onRequestFeedback: (blockId: string, reflection: string) => Promise<{ passed: boolean; feedback: string }>;
  onContinue: () => void;
  continueLabel: string;
}) {
  const [reflection, setReflection] = useState(content.studentReflection ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const handleChange = (value: string) => {
    setReflection(value);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => onSave(block.id, { ...content, studentReflection: value }), SAVE_DEBOUNCE_MS);
  };

  const approved = block.status === "passed";
  const passed = approved || outcome?.passed === true;
  // Latest verdict wins: the fresh call, else the stored one (survives reload).
  const shownFeedback = outcome?.feedback ?? block.last_feedback ?? null;

  const requestFeedback = async () => {
    if (!isActive || busy || approved || !reflection.trim()) return;
    // Flush any pending debounced save so both paths agree on the text —
    // the backend grades and persists `reflection` as given regardless.
    if (timer.current) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    setBusy(true);
    setError(null);
    try {
      setOutcome(await onRequestFeedback(block.id, reflection));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="notebook-block-body space-y-4">
      {/* Bitácora de cierre: encabezado cálido */}
      <div className="closure-card-header">
        <div className="closure-badge">
          <IconSparkles />
          <span>Bitácora de cierre</span>
        </div>
        <span className="closure-subtitle">Consolida tu aprendizaje en tu cuaderno</span>
      </div>

      {content.predictionComparison && (
        <div className="closure-evolution-container">
          <div className="closure-evolution-grid">
            <div className="closure-evolution-card starting">
              <div className="closure-card-head">
                <span className="closure-pill">Tu intuición inicial</span>
                <span className="closure-tag">Punto de partida</span>
              </div>
              <div className="closure-card-body">
                <InlineText text={content.predictionComparison.initialPrediction} />
              </div>
            </div>

            <div className="closure-evolution-card demonstrated">
              <div className="closure-card-head">
                <span className="closure-pill">Lo que lograste hoy</span>
                <span className="closure-tag">En la práctica</span>
              </div>
              <div className="closure-card-body">
                <InlineText text={content.predictionComparison.finalResult} />
              </div>
            </div>
          </div>

          {content.predictionComparison.contrastNarrative && (
            <div className="closure-evolution-narrative">
              <div className="closure-narrative-icon">
                <IconLightbulb />
              </div>
              <div className="closure-narrative-text">
                <strong className="closure-narrative-title">El clic de esta sesión:</strong>
                <InlineText text={content.predictionComparison.contrastNarrative} />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Tarea de síntesis personal */}
      <div className="closure-prompt-box">
        <div className="closure-prompt-header">
          <IconPencil />
          <span>Tu reflexión en el cuaderno</span>
        </div>
        <RichText className="closure-prompt-text" text={content.synthesisTask} />

        {content.selfEvaluationChecklist.length > 0 && (
          <div className="closure-checklist-wrap">
            <span className="closure-checklist-title">Puntos clave a tener en cuenta:</span>
            <ul className="closure-checklist">
              {content.selfEvaluationChecklist.map((c) => (
                <li key={c} className="closure-checklist-item">
                  <span className="closure-check-bullet">✓</span>
                  <div className="flex-1 min-w-0">
                    <InlineText text={c} />
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}

        <textarea
          value={reflection}
          onChange={(e) => handleChange(e.target.value)}
          placeholder="Escribe en 2 o 3 frases qué fue lo que te hizo clic o qué regla te llevas más clara..."
          rows={3}
          disabled={approved}
          className="closure-textarea"
        />
      </div>

      {passed && <ConnectStep />}

      <div className="closure-actions">
        {!approved && (
          <GateSubmitButton
            label="Obtener feedback"
            busy={busy}
            disabled={!isActive || !reflection.trim()}
            disabledReason={!isActive ? "Este cierre ya no está activo." : "Escribe tu reflexión antes de pedir feedback."}
            onClick={() => void requestFeedback()}
          />
        )}
        {passed && (
          <button className="btn-primary" onClick={onContinue}>
            {continueLabel}
          </button>
        )}
      </div>

      <GateProgress busy={busy} label="Revisando tu reflexión" />

      {error && <p className="hint hint-warn">{error}</p>}

      {!busy && shownFeedback && (
        <div className={`closure-feedback-box ${passed ? "is-ok" : "is-warn"}`}>
          <div className="closure-feedback-header">
            {passed ? <IconCheck /> : <IconTarget />}
            <span>{passed ? "¡Reflexión aprobada y clase completada!" : "Orientación de tu mentor:"}</span>
          </div>
          <div className="closure-feedback-body">
            <InlineText text={shownFeedback} />
          </div>
        </div>
      )}
    </div>
  );
}
