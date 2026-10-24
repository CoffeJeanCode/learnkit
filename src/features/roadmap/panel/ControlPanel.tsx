import { useEffect, useMemo, useState } from "react";
import type { ContextAnswer, ContextQuestionSet, ProfileBasics } from "../../../lib/schemas";
import {
  type AnswersDraft,
  type BasicsDraft,
  EXAMPLES,
  answerErrors,
  basicsErrors,
  basicsToDraft,
  emptyBasics,
  formatWeekly,
  toAnswers,
  toBasicsPayload,
  visibleQuestions,
  weeklyHours,
} from "../profileFormModel";
import { AnswerCards } from "./AnswerCards";
import { RhythmTile } from "./RhythmTile";
import { WeeksDial } from "./WeeksDial";
import { DEFAULT_ORDER, TILE_TITLES, type TileId, browserStore, loadOrder, moveTile, saveOrder, tileStatus } from "./panelModel";

/** Sensible starting values, so the learner only has to TYPE two things; weeks and
 *  time are changed only if the default does not fit. */
export const STARTING_DRAFT: BasicsDraft = { ...emptyBasics(), weeks: "8", hoursPerDay: "1" };

const QUESTIONS: Record<"topic" | "goal", { label: string; placeholder: string }> = {
  topic: { label: "¿Qué quieres aprender?", placeholder: "Ej.: mutaciones genéticas, Linux, fotografía…" },
  goal: { label: "¿Qué quieres lograr con eso?", placeholder: "Ej.: explicar cómo ocurre una mutación" },
};

/**
 * Creating the plan as a short board instead of a form. Topic and goal are
 * typed in place; weeks (a dial) and time (day switches + an hours slider)
 * start with a default and open only to change it. Step 2 turns the topic's
 * questions into cards. It fills the same drafts the old form did and hands
 * them to the same callbacks, so the backend contract is unchanged. Which step
 * shows comes from the SESSION (`questions` present = step 2), as before.
 *
 * Rearranging the rows is available but hidden behind "Personalizar".
 */
export function ControlPanel({
  initialBasics,
  questions,
  busy,
  serverError,
  onPropose,
  onSubmit,
}: {
  initialBasics: ProfileBasics | null;
  questions: ContextQuestionSet | null;
  busy: boolean;
  serverError: string | null;
  onPropose: (basics: ProfileBasics) => void;
  onSubmit: (answers: ContextAnswer[]) => void;
}) {
  const [basics, setBasics] = useState<BasicsDraft>(() => (initialBasics ? basicsToDraft(initialBasics) : STARTING_DRAFT));
  const [answers, setAnswers] = useState<AnswersDraft>({});
  const [editing, setEditing] = useState(false);
  const [attempted, setAttempted] = useState(false);
  const [open, setOpen] = useState<TileId | null>(null);
  const [customizing, setCustomizing] = useState(false);
  const [order, setOrder] = useState<TileId[]>(() => loadOrder(browserStore()));

  const questionsKey = questions ? JSON.stringify(questions) : "";
  useEffect(() => {
    setAnswers({});
    setEditing(false);
    setAttempted(false);
  }, [questionsKey]);

  const set = (patch: Partial<BasicsDraft>) => setBasics((b) => ({ ...b, ...patch }));
  const errors = useMemo(() => basicsErrors(basics), [basics]);
  const move = (id: TileId, dir: -1 | 1) => {
    const next = moveTile(order, id, dir);
    setOrder(next);
    saveOrder(browserStore(), next);
  };
  const resetOrder = () => {
    setOrder([...DEFAULT_ORDER]);
    saveOrder(browserStore(), DEFAULT_ORDER);
  };
  const isDefaultOrder = order.every((id, i) => id === DEFAULT_ORDER[i]);

  const step2 = questions !== null && !editing;

  if (!step2) {
    const next = () => {
      setAttempted(true);
      const payload = toBasicsPayload(basics);
      if (payload) onPropose(payload);
    };
    const err = (key: string) =>
      attempted && errors[key] ? (
        <span className="field-error" role="alert">
          {errors[key]}
        </span>
      ) : null;
    const weekly = weeklyHours(basics.hoursPerDay, basics.daysPerWeek);
    const value: Record<"weeks" | "rhythm", string> = {
      weeks: basics.weeks.trim() ? `${basics.weeks} semanas` : "Elegir",
      rhythm: weekly === null ? "Elegir" : formatWeekly(weekly),
    };

    const row = (id: TileId, i: number) => {
      const hasError = tileStatus(id, basics, attempted) === "error";
      const mover = customizing && (
        <span className="cp-move">
          <button type="button" className="btn-quiet" aria-label={`Mover «${TILE_TITLES[id]}» antes`} disabled={i === 0} onClick={() => move(id, -1)}>
            ↑
          </button>
          <button type="button" className="btn-quiet" aria-label={`Mover «${TILE_TITLES[id]}» después`} disabled={i === order.length - 1} onClick={() => move(id, 1)}>
            ↓
          </button>
        </span>
      );

      if (id === "topic" || id === "goal") {
        const field = id === "topic" ? "topic" : "targetGoal";
        const q = QUESTIONS[id];
        return (
          <div key={id} className={`cp-row${hasError ? " cp-error" : ""}`}>
            <label className="pf-field">
              <span className="pf-label">{q.label}</span>
              <input value={basics[field]} onChange={(e) => set({ [field]: e.target.value })} placeholder={q.placeholder} disabled={busy} autoFocus={id === order[0]} />
              {err(field)}
            </label>
            {mover}
          </div>
        );
      }

      const expanded = open === id;
      return (
        <div key={id} className={`cp-row${hasError ? " cp-error" : ""}`}>
          <div className="cp-line">
            <button type="button" className="cp-toggle" aria-expanded={expanded} onClick={() => setOpen(expanded ? null : id)} disabled={busy}>
              <span className="pf-label">{TILE_TITLES[id]}</span>
              <span className="cp-value">{value[id as "weeks" | "rhythm"]}</span>
              <span className="cp-chevron">{expanded ? "Cerrar" : "Cambiar"}</span>
            </button>
            {mover}
          </div>
          {expanded && (
            <div className="cp-body">
              {id === "weeks" ? (
                <WeeksDial value={basics.weeks} onChange={(weeks) => set({ weeks })} disabled={busy} />
              ) : (
                <RhythmTile daysPerWeek={basics.daysPerWeek} hoursPerDay={basics.hoursPerDay} onChange={set} disabled={busy} error={errors.weekly ?? (attempted ? (errors.hoursPerDay ?? errors.daysPerWeek) : undefined)} />
              )}
              {err("weeks")}
            </div>
          )}
        </div>
      );
    };

    return (
      <form
        className="control-panel"
        onSubmit={(e) => {
          e.preventDefault();
          next();
        }}
        noValidate
      >
        <div className="cp-list">{order.map(row)}</div>

        <div className="cp-examples">
          <span className="hint">¿Sin ideas? Prueba:</span>
          {EXAMPLES.map((ex) => (
            <button key={ex.label} type="button" className="cp-example" onClick={() => setBasics(ex.draft)} disabled={busy}>
              {ex.label}
            </button>
          ))}
        </div>

        {serverError && (
          <div className="alert error" role="alert">
            <span className="alert-text">{serverError}</span>
          </div>
        )}

        <div className="row">
          <button className="btn-primary" type="submit" disabled={busy}>
            {busy ? "Pensando qué preguntarte…" : "Continuar"}
          </button>
          <button type="button" className="btn-quiet" onClick={() => setCustomizing((c) => !c)} disabled={busy}>
            {customizing ? "Listo" : "Personalizar"}
          </button>
          {customizing && !isDefaultOrder && (
            <button type="button" className="btn-quiet" onClick={resetOrder} disabled={busy}>
              Restablecer orden
            </button>
          )}
          {attempted && Object.keys(errors).length > 0 && <span className="hint">Revisa lo marcado.</span>}
        </div>
      </form>
    );
  }

  // ---- Step 2: the topic's questions as cards ------------------------------------------------------------
  const answerErrs = answerErrors(questions, answers);
  const submit = () => {
    setAttempted(true);
    const payload = toAnswers(questions, answers);
    if (payload) onSubmit(payload);
  };

  return (
    <form
      className="control-panel"
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

      <AnswerCards
        questions={visibleQuestions(questions, answers)}
        answers={answers}
        errors={answerErrs}
        attempted={attempted}
        disabled={busy}
        onAnswer={(id, value) => setAnswers((a) => ({ ...a, [id]: value }))}
      />

      {serverError && (
        <div className="alert error" role="alert">
          <span className="alert-text">{serverError}</span>
        </div>
      )}

      <div className="row">
        <button className="btn-primary" type="submit" disabled={busy}>
          {busy ? "Armando tu plan…" : "Armar mi plan"}
        </button>
        {attempted && Object.keys(answerErrs).length > 0 && <span className="hint">Revisa lo marcado.</span>}
      </div>
    </form>
  );
}
