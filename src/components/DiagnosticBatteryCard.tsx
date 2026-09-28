import { useEffect, useState } from "react";
import type { DiagnosticBatteryState } from "../lib/schemas";
import { getCourseDiagnosticBattery, saveDiagnosticBatteryAnswers, tauriError } from "../lib/tauri";
import { InlineText } from "../features/notebook/blocks/RichText";

const DIMENSION_LABEL: Record<string, string> = {
  intuition: "Intuición",
  mechanics: "Mecánica",
  critical_case: "Caso crítico",
  boundary: "Frontera",
};

/**
 * The roadmap's calibration battery — generated once, alongside the
 * syllabus (never as a notebook block, see `domain::notebook::
 * DiagnosticBattery` in Rust), so it lives on the PLAN screen, not inside a
 * class. Answers persist per-course (`saveDiagnosticBatteryAnswers`) and
 * feed back into how later classes get personalized (see
 * `NotebookService::generate_class_notebook`'s `diagnosticProfile`).
 */
export function DiagnosticBatteryCard({ courseId }: { courseId: string }) {
  const [state, setState] = useState<DiagnosticBatteryState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getCourseDiagnosticBattery(courseId)
      .then((s) => {
        if (!cancelled) setState(s);
      })
      .catch((e) => {
        if (!cancelled) setError(tauriError(e).message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [courseId]);

  const choose = (index: number, option: string) => {
    if (!state) return;
    const next = { ...state.answers, [String(index)]: option };
    setState({ ...state, answers: next });
    saveDiagnosticBatteryAnswers(courseId, next).catch((e) => setError(tauriError(e).message));
  };

  if (loading) return null;
  if (error) return <p className="hint">{error}</p>;
  if (!state) return null; // this course predates the battery, or never got one

  return (
    <div className="card diagnostic-battery-card">
      <div className="card-head">
        <h3>Antes de empezar</h3>
        <span className="badge">calibrando</span>
      </div>
      <p className="hint">{state.goalAlignment}</p>
      <ol className="diagnostic-question-list">
        {state.questions.map((q, i) => {
          const chosen = state.answers[String(i)];
          return (
            <li key={i} className="diagnostic-question">
              <div className="diagnostic-question-head">
                <span className="badge">{DIMENSION_LABEL[q.dimension] ?? q.dimension}</span>
                <p className="block-question">
                  <InlineText text={q.prompt} />
                </p>
              </div>
              <div className="prediction-options">
                {q.options.map((opt) => (
                  <button
                    key={opt}
                    className={chosen === opt ? "btn-primary" : ""}
                    onClick={() => choose(i, opt)}
                    disabled={chosen !== undefined}
                  >
                    <InlineText text={opt} />
                  </button>
                ))}
              </div>
              {chosen && (
                <p className="hint">
                  {chosen === q.correctOption ? "Correcto. " : `La respuesta esperada era "${q.correctOption}". `}
                  {q.diagnosticInsight}
                </p>
              )}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
