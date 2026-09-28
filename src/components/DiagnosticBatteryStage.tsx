import { useEffect, useState } from "react";
import type { DiagnosticBatteryState } from "../lib/schemas";
import { InlineText } from "../features/notebook/blocks/RichText";

const DIMENSION_LABEL: Record<string, string> = {
  intuition: "Intuición",
  mechanics: "Mecánica",
  critical_case: "Caso crítico",
  boundary: "Frontera",
};

/**
 * The live Diagnostic stage, rendered inline in the chat — NOT a notebook
 * block, NOT a post-seal recap (see `DiagnosticBatteryCard` for that). This
 * is the one genuine wait in the whole autonomous flow: the student answers
 * here, in real time, BEFORE the syllabus is generated (see
 * `RoadmapService::answer_diagnostic_question`). Picks are revealed via
 * LOCAL state (see below) rather than `battery.answers` directly, so
 * feedback shows immediately on click regardless of how long the backend
 * round trip takes. There is deliberately NO skip: if the battery is up,
 * the model judged it necessary for this student, so it has to be answered.
 */
export function DiagnosticBatteryStage({
  battery,
  onAnswer,
  busy,
}: {
  battery: DiagnosticBatteryState;
  onAnswer: (index: number, answer: string) => void;
  busy: boolean;
}) {
  // BUG THIS FIXED: reading `chosen` straight from `battery.answers` meant
  // the LAST question never showed feedback — answering it triggers a real
  // model turn (finish_diagnostic_stage) on the backend, so the session
  // (and this card's `battery` prop) doesn't update until that whole turn
  // resolves, by which point the battery is already fully answered and the
  // card unmounts immediately in favor of the proposed plan. Local state
  // reveals the pick + feedback the INSTANT the student clicks, independent
  // of how long the round trip takes — seeded from `battery.answers` so a
  // restored/reopened session still renders already-answered questions.
  const [localAnswers, setLocalAnswers] = useState<Record<string, string>>(battery.answers);
  useEffect(() => {
    setLocalAnswers((prev) => ({ ...battery.answers, ...prev }));
  }, [battery]);

  const choose = (index: number, option: string) => {
    setLocalAnswers((prev) => ({ ...prev, [String(index)]: option }));
    onAnswer(index, option);
  };

  return (
    <div className="card diagnostic-battery-card">
      <div className="card-head">
        <h3>Antes de armar tu plan</h3>
        <span className="badge">calibrando</span>
      </div>
      <p className="hint">{battery.goalAlignment}</p>
      <ol className="diagnostic-question-list">
        {battery.questions.map((q, i) => {
          const chosen = localAnswers[String(i)];
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
                    disabled={busy || chosen !== undefined}
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
