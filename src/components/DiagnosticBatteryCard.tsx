import { useEffect, useState } from "react";
import type { DiagnosticBatteryState } from "../lib/schemas";
import { getCourseDiagnosticBattery, saveDiagnosticBatteryAnswers, tauriError } from "../lib/tauri";
import { DiagnosticQuestionList } from "./DiagnosticQuestionList";

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
      <DiagnosticQuestionList questions={state.questions} answers={state.answers} onChoose={choose} />
    </div>
  );
}
