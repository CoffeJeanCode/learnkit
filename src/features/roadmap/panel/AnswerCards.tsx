import { type ContextQuestion, UNKNOWN_ANSWER } from "../../../lib/schemas";
import { LIMITS } from "../profileFormModel";

/**
 * One big card per question; its options are cards to pick (never a list of
 * small chips), "No lo sé" is a card of its own, and a text question is a card
 * with one short input. Follow-ups appear because the parent passes them in
 * `questions` (see `visibleQuestions`).
 */
export function AnswerCards({
  questions,
  answers,
  errors,
  attempted,
  disabled,
  onAnswer,
}: {
  questions: ContextQuestion[];
  answers: Record<string, string>;
  errors: Record<string, string>;
  attempted: boolean;
  disabled?: boolean;
  onAnswer: (id: string, value: string) => void;
}) {
  return (
    <div className="answer-cards">
      {questions.map((q) => (
        <fieldset key={q.id} className="answer-card pf-fieldset" disabled={disabled}>
          <legend className="pf-label">{q.label}</legend>
          {q.kind === "choice" ? (
            <div className="card-deck">
              {[...q.options.map((o) => o.label), UNKNOWN_ANSWER].map((label) => {
                const selected = answers[q.id] === label;
                return (
                  <button
                    key={label}
                    type="button"
                    className={`pick-card${selected ? " selected" : ""}${label === UNKNOWN_ANSWER ? " pick-unknown" : ""}`}
                    aria-pressed={selected}
                    onClick={() => onAnswer(q.id, label)}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          ) : (
            <input value={answers[q.id] ?? ""} onChange={(e) => onAnswer(q.id, e.target.value)} placeholder="Opcional" maxLength={LIMITS.textAnswerMax} />
          )}
          {attempted && errors[`q:${q.id}`] && (
            <span className="field-error" role="alert">
              {errors[`q:${q.id}`]}
            </span>
          )}
        </fieldset>
      ))}
    </div>
  );
}
