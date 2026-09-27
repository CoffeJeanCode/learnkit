import { useEffect, useRef, useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";

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
    <div className="notebook-block-body">
      {content.predictionComparison && (
        <div className="prediction-comparison">
          <p className="hint">
            <strong>Predijiste:</strong> {content.predictionComparison.initialPrediction}
          </p>
          <p className="hint">
            <strong>Lo que demostraste:</strong> {content.predictionComparison.finalResult}
          </p>
          <p>{content.predictionComparison.contrastNarrative}</p>
        </div>
      )}
      <p className="block-question">{content.synthesisTask}</p>
      <ul className="dod-list">
        {content.selfEvaluationChecklist.map((c) => (
          <li key={c}>{c}</li>
        ))}
      </ul>
      <textarea
        value={reflection}
        onChange={(e) => handleChange(e.target.value)}
        placeholder="Con tus propias palabras…"
        rows={3}
        disabled={approved}
      />
      <div className="closure-actions">
        {!approved && (
          <button className="btn-primary" onClick={() => void requestFeedback()} disabled={!isActive || busy || !reflection.trim()}>
            {busy ? "Calificando…" : "Obtener feedback"}
          </button>
        )}
        {passed && (
          <button className="btn-primary" onClick={onContinue}>
            {continueLabel}
          </button>
        )}
      </div>
      {busy && <p className="hint">Revisando tu cierre…</p>}
      {error && <p className="hint hint-warn">{error}</p>}
      {!busy && shownFeedback && <p className={passed ? "hint hint-ok" : "hint hint-warn"}>{shownFeedback}</p>}
    </div>
  );
}
