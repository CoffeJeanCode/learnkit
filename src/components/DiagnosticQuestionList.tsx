import type { DiagnosticBatteryState } from "../lib/schemas";
import { InlineText } from "../features/notebook/blocks/RichText";

const DIMENSION_LABEL: Record<string, string> = {
  intuition: "Intuición",
  mechanics: "Mecánica",
  critical_case: "Caso crítico",
  boundary: "Frontera",
};

/**
 * The answer recorded when the student admits they don't know. It is never
 * one of the generated `options`, so the backend scores it like any other
 * miss (`DiagnosticBatteryState::profile_summary` / `calibration::
 * dimension_results`) — which is exactly right: the battery measures WHAT
 * the student knows, and an honest "no sé" is a cleaner signal than a guess.
 */
export const DONT_KNOW_ANSWER = "No sé";

/**
 * The question list shared by the live Diagnostic stage and the plan-screen
 * recap. Deliberately shows NO correctness feedback: this is a calibration,
 * not a quiz — revealing the key would teach the answer before the plan is
 * built on it and nudge the student toward guessing instead of being honest.
 */
export function DiagnosticQuestionList({
  questions,
  answers,
  onChoose,
  disabled = false,
}: {
  questions: DiagnosticBatteryState["questions"];
  answers: Record<string, string>;
  onChoose: (index: number, option: string) => void;
  disabled?: boolean;
}) {
  return (
    <ol className="diagnostic-question-list">
      {questions.map((q, i) => {
        const chosen = answers[String(i)];
        const locked = disabled || chosen !== undefined;
        return (
          <li key={i} className="diagnostic-question">
            <div className="diagnostic-question-head">
              <span className="badge">{DIMENSION_LABEL[q.dimension] ?? q.dimension}</span>
              <p className="block-question">
                <InlineText text={q.prompt} />
              </p>
            </div>
            <div className="prediction-options diagnostic-options">
              {q.options.map((opt) => (
                <button
                  key={opt}
                  className={chosen === opt ? "btn-primary" : ""}
                  onClick={() => onChoose(i, opt)}
                  disabled={locked}
                >
                  <InlineText text={opt} />
                </button>
              ))}
              <button
                className={`diagnostic-dont-know${chosen === DONT_KNOW_ANSWER ? " btn-primary" : ""}`}
                onClick={() => onChoose(i, DONT_KNOW_ANSWER)}
                disabled={locked}
              >
                {DONT_KNOW_ANSWER}
              </button>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
