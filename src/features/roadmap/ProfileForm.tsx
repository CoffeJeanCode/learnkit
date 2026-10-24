import { useEffect, useState } from "react";
import type { ContextAnswer, ContextQuestionSet, ProfileBasics } from "../../lib/schemas";
import {
  type AnswersDraft,
  type BasicsDraft,
  EXAMPLES,
  LIMITS,
  answerErrors,
  basicsErrors,
  basicsToDraft,
  emptyBasics,
  formatWeekly,
  toAnswers,
  toBasicsPayload,
  visibleQuestions,
  weeklyHours,
} from "./profileFormModel";
import { UNKNOWN_ANSWER } from "../../lib/schemas";

/**
 * The profile as a short form in TWO steps, instead of a chat:
 *
 *  1. what the learner types — topic, goal, weeks, hours;
 *  2. a few questions written for THAT topic (what they already know, what they
 *     have to practice with, what limits them). Biology of mutations asks about
 *     DNA; photography asks about the camera. A model proposes them once, the
 *     backend validates them, and the starting level is deduced from the
 *     answers, so nobody picks "principiante/intermedio" from a menu.
 *
 * Which step shows comes from the SESSION (`questions` present = step 2), so
 * reopening a session resumes where the learner left off.
 */
export function ProfileForm({
  initialBasics,
  questions,
  busy,
  serverError,
  onPropose,
  onSubmit,
}: {
  /** What the server kept from step 1, if anything. */
  initialBasics: ProfileBasics | null;
  /** The server's questions for this topic; `null` = still at step 1. */
  questions: ContextQuestionSet | null;
  busy: boolean;
  /** A refusal from the backend (e.g. an answer that is not an option). */
  serverError: string | null;
  onPropose: (basics: ProfileBasics) => void;
  onSubmit: (answers: ContextAnswer[]) => void;
}) {
  const [basics, setBasics] = useState<BasicsDraft>(() => (initialBasics ? basicsToDraft(initialBasics) : emptyBasics()));
  const [answers, setAnswers] = useState<AnswersDraft>({});
  // Step 1 -> "Cambiar" returns to it without losing what was typed.
  const [editing, setEditing] = useState(false);
  // Messages appear only after the first attempt: no red form on first sight.
  const [attempted, setAttempted] = useState(false);

  // A new set of questions (first proposal, or a re-proposal after editing
  // step 1) starts step 2 clean.
  const questionsKey = questions ? JSON.stringify(questions) : "";
  useEffect(() => {
    setAnswers({});
    setEditing(false);
    setAttempted(false);
  }, [questionsKey]);

  const step2 = questions !== null && !editing;

  if (!step2) {
    const errors = basicsErrors(basics);
    const weekly = weeklyHours(basics.hoursPerDay, basics.daysPerWeek);
    // The weekly total is shown as soon as it can be computed: it is the live check the learner needs.
    const shown = (key: string) => (attempted || key === "weekly" ? errors[key] : undefined);
    const set = (patch: Partial<BasicsDraft>) => setBasics((b) => ({ ...b, ...patch }));
    const field = (key: string) => {
      const msg = shown(key);
      return msg ? (
        <span className="field-error" role="alert">
          {msg}
        </span>
      ) : null;
    };
    const next = () => {
      setAttempted(true);
      const payload = toBasicsPayload(basics);
      if (payload) onPropose(payload);
    };
    return (
      <form
        className="profile-form"
        onSubmit={(e) => {
          e.preventDefault();
          next();
        }}
        noValidate
      >
        <p className="hint">Parte de un ejemplo y cámbialo, o escribe lo tuyo:</p>
        <div className="example-prompts">
          {EXAMPLES.map((ex) => (
            <button key={ex.label} type="button" className={`example-chip example-chip-${ex.tone}`} onClick={() => setBasics(ex.draft)} disabled={busy}>
              {ex.label}
            </button>
          ))}
        </div>

        <label className="pf-field">
          <span className="pf-label">¿Qué quieres aprender?</span>
          <input value={basics.topic} onChange={(e) => set({ topic: e.target.value })} placeholder="Ej.: mutaciones genéticas, Linux, fotografía…" disabled={busy} autoFocus />
          {field("topic")}
        </label>

        <label className="pf-field">
          <span className="pf-label">¿Qué quieres lograr con eso?</span>
          <input value={basics.targetGoal} onChange={(e) => set({ targetGoal: e.target.value })} placeholder="Ej.: explicar cómo ocurre una mutación" disabled={busy} />
          {field("targetGoal")}
        </label>

        <div className="pf-row pf-row-3">
          <label className="pf-field">
            <span className="pf-label">¿Cuántas semanas?</span>
            <input inputMode="numeric" value={basics.weeks} onChange={(e) => set({ weeks: e.target.value })} placeholder={`${LIMITS.weeksMin}–${LIMITS.weeksMax}`} disabled={busy} />
            {field("weeks")}
          </label>
          <label className="pf-field">
            <span className="pf-label">Horas por día</span>
            <input inputMode="decimal" value={basics.hoursPerDay} onChange={(e) => set({ hoursPerDay: e.target.value })} placeholder="Ej.: 1 o 0,5" disabled={busy} />
            {field("hoursPerDay")}
          </label>
          <label className="pf-field">
            <span className="pf-label">Días por semana</span>
            <input inputMode="numeric" value={basics.daysPerWeek} onChange={(e) => set({ daysPerWeek: e.target.value })} placeholder={`${LIMITS.daysMin}–${LIMITS.daysMax}`} disabled={busy} />
            {field("daysPerWeek")}
          </label>
        </div>
        {weekly !== null && !errors.weekly && <p className="hint pf-total">Son {formatWeekly(weekly)}.</p>}
        {field("weekly")}

        {serverError && (
          <div className="alert error" role="alert">
            <span className="alert-text">{serverError}</span>
          </div>
        )}

        <div className="row">
          <button className="btn-primary" type="submit" disabled={busy}>
            {busy ? "Pensando qué preguntarte…" : "Continuar"}
          </button>
          {attempted && Object.keys(errors).length > 0 && <span className="hint">Revisa los campos marcados.</span>}
        </div>
      </form>
    );
  }

  // ---- Step 2 -------------------------------------------------------------------------
  const errors = answerErrors(questions, answers);
  const submit = () => {
    setAttempted(true);
    const payload = toAnswers(questions, answers);
    if (payload) onSubmit(payload);
  };
  const setAnswer = (id: string, value: string) => setAnswers((a) => ({ ...a, [id]: value }));

  return (
    <form
      className="profile-form"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
      noValidate
    >
      <div className="pf-recap">
        <span>
          <strong>{basics.topic.trim()}</strong> · {basics.weeks} semanas · {formatWeekly(weeklyHours(basics.hoursPerDay, basics.daysPerWeek) ?? 0)}
        </span>
        <button type="button" className="btn-quiet" onClick={() => setEditing(true)} disabled={busy}>
          Cambiar
        </button>
      </div>
      <p className="hint">Unas preguntas sobre tu tema, para no explicarte lo que ya sabes ni suponer lo que no tienes. Si no sabes algo, elige «{UNKNOWN_ANSWER}»: no pasa nada.</p>

      {visibleQuestions(questions, answers).map((q) => (
        <fieldset key={q.id} className="pf-field pf-fieldset" disabled={busy}>
          <legend className="pf-label">{q.label}</legend>
          {q.kind === "choice" ? (
            <div className="chip-row">
              {q.options.map((opt) => (
                <button
                  key={opt.label}
                  type="button"
                  className={`chip-choice${answers[q.id] === opt.label ? " selected" : ""}`}
                  aria-pressed={answers[q.id] === opt.label}
                  onClick={() => setAnswer(q.id, opt.label)}
                >
                  {opt.label}
                </button>
              ))}
              <button
                type="button"
                className={`chip-choice chip-unknown${answers[q.id] === UNKNOWN_ANSWER ? " selected" : ""}`}
                aria-pressed={answers[q.id] === UNKNOWN_ANSWER}
                onClick={() => setAnswer(q.id, UNKNOWN_ANSWER)}
              >
                {UNKNOWN_ANSWER}
              </button>
            </div>
          ) : (
            <input value={answers[q.id] ?? ""} onChange={(e) => setAnswer(q.id, e.target.value)} placeholder="Opcional" maxLength={LIMITS.textAnswerMax} />
          )}
          {attempted && errors[`q:${q.id}`] && (
            <span className="field-error" role="alert">
              {errors[`q:${q.id}`]}
            </span>
          )}
        </fieldset>
      ))}

      {serverError && (
        <div className="alert error" role="alert">
          <span className="alert-text">{serverError}</span>
        </div>
      )}

      <div className="row">
        <button className="btn-primary" type="submit" disabled={busy}>
          {busy ? "Armando tu plan…" : "Armar mi plan"}
        </button>
        {attempted && Object.keys(errors).length > 0 && <span className="hint">Revisa los campos marcados.</span>}
      </div>
    </form>
  );
}
