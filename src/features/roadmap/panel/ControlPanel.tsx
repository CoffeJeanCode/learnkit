import { useEffect, useMemo, useState } from "react";
import type { ContextAnswer, ContextQuestionSet, ProfileBasics } from "../../../lib/schemas";
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
} from "../profileFormModel";
import { AnswerCards } from "./AnswerCards";
import { RhythmTile } from "./RhythmTile";
import { WeeksDial } from "./WeeksDial";
import {
  DEFAULT_ORDER,
  TILE_TITLES,
  type TileId,
  type TileStatus,
  browserStore,
  loadOrder,
  missionProgress,
  moveTile,
  saveOrder,
  tileStatus,
} from "./panelModel";

const STATUS_MARK: Record<TileStatus, string> = { ready: "✓", error: "!", empty: "" };

/**
 * Creating the plan as a control panel instead of a form. Mission 1 is a board
 * of tiles (topic and goal are typed right on the board; weeks is a dial; time
 * is day switches plus an hours slider); mission 2 turns the topic's questions
 * into cards. It fills the same drafts the old form did and hands them to the
 * same callbacks, so the backend contract is unchanged. Which step shows comes
 * from the SESSION (`questions` present = mission 2), as before.
 *
 * The board is personalizable: tiles can be moved, and the order is remembered.
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
  const [basics, setBasics] = useState<BasicsDraft>(() => (initialBasics ? basicsToDraft(initialBasics) : emptyBasics()));
  const [answers, setAnswers] = useState<AnswersDraft>({});
  const [editing, setEditing] = useState(false);
  const [attempted, setAttempted] = useState(false);
  const [open, setOpen] = useState<TileId | null>(null);
  const [order, setOrder] = useState<TileId[]>(() => loadOrder(browserStore()));

  const questionsKey = questions ? JSON.stringify(questions) : "";
  useEffect(() => {
    setAnswers({});
    setEditing(false);
    setAttempted(false);
  }, [questionsKey]);

  const set = (patch: Partial<BasicsDraft>) => setBasics((b) => ({ ...b, ...patch }));
  const errors = useMemo(() => basicsErrors(basics), [basics]);
  const progress = missionProgress(basics);
  const move = (id: TileId, dir: -1 | 1) => {
    const next = moveTile(order, id, dir);
    setOrder(next);
    saveOrder(browserStore(), next);
  };
  const resetOrder = () => {
    setOrder([...DEFAULT_ORDER]);
    saveOrder(browserStore(), DEFAULT_ORDER);
  };

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
    const summary: Record<TileId, string> = {
      topic: basics.topic.trim() || "Sin elegir",
      goal: basics.targetGoal.trim() || "Sin elegir",
      weeks: basics.weeks.trim() ? `${basics.weeks} semanas` : "Sin elegir",
      rhythm: (() => {
        const w = weeklyHours(basics.hoursPerDay, basics.daysPerWeek);
        return w === null ? "Sin elegir" : formatWeekly(w);
      })(),
    };

    const body = (id: TileId) => {
      switch (id) {
        case "topic":
          return (
            <>
              <input
                value={basics.topic}
                onChange={(e) => set({ topic: e.target.value })}
                placeholder="Ej.: mutaciones genéticas, Linux, fotografía…"
                disabled={busy}
                aria-label={TILE_TITLES.topic}
                autoFocus
              />
              {err("topic")}
            </>
          );
        case "goal":
          return (
            <>
              <input
                value={basics.targetGoal}
                onChange={(e) => set({ targetGoal: e.target.value })}
                placeholder="Ej.: explicar cómo ocurre una mutación"
                disabled={busy}
                aria-label={TILE_TITLES.goal}
              />
              {err("targetGoal")}
            </>
          );
        case "weeks":
          return (
            <>
              <WeeksDial value={basics.weeks} onChange={(weeks) => set({ weeks })} disabled={busy} />
              {err("weeks")}
            </>
          );
        case "rhythm":
          return (
            <RhythmTile
              daysPerWeek={basics.daysPerWeek}
              hoursPerDay={basics.hoursPerDay}
              onChange={set}
              disabled={busy}
              error={errors.weekly ?? (attempted ? (errors.hoursPerDay ?? errors.daysPerWeek) : undefined)}
            />
          );
      }
    };

    // The two typed tiles are always open; weeks and time open their mini-game on demand.
    const alwaysOpen = (id: TileId) => id === "topic" || id === "goal";

    return (
      <form
        className="control-panel"
        onSubmit={(e) => {
          e.preventDefault();
          next();
        }}
        noValidate
      >
        <div className="cp-meter" role="status">
          <span className="pf-label">Misión 1 · {progress.done} de {progress.total}</span>
          <div className="cp-meter-bar" aria-hidden="true">
            <div className="cp-meter-fill" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
          </div>
        </div>

        <p className="hint">Parte de un ejemplo y cámbialo, o completa cada mosaico:</p>
        <div className="example-prompts">
          {EXAMPLES.map((ex) => (
            <button key={ex.label} type="button" className={`example-chip example-chip-${ex.tone}`} onClick={() => setBasics(ex.draft)} disabled={busy}>
              {ex.label}
            </button>
          ))}
        </div>

        <div className="cp-grid">
          {order.map((id, i) => {
            const status = tileStatus(id, basics, attempted);
            const expanded = alwaysOpen(id) || open === id;
            return (
              <section key={id} className={`cp-tile cp-${status}${alwaysOpen(id) ? " cp-wide" : ""}`} aria-label={TILE_TITLES[id]}>
                <header className="cp-tile-head">
                  {alwaysOpen(id) ? (
                    <span className="pf-label">{TILE_TITLES[id]}</span>
                  ) : (
                    <button type="button" className="cp-toggle" aria-expanded={expanded} onClick={() => setOpen(expanded ? null : id)} disabled={busy}>
                      <span className="pf-label">{TILE_TITLES[id]}</span>
                      <span className="cp-summary">{summary[id]}</span>
                    </button>
                  )}
                  <span className={`cp-status cp-status-${status}`} aria-label={status === "ready" ? "Listo" : status === "error" ? "Falta revisar" : "Pendiente"}>
                    {STATUS_MARK[status]}
                  </span>
                  <span className="cp-move">
                    <button type="button" className="btn-quiet" aria-label={`Mover «${TILE_TITLES[id]}» antes`} disabled={i === 0} onClick={() => move(id, -1)}>
                      ←
                    </button>
                    <button type="button" className="btn-quiet" aria-label={`Mover «${TILE_TITLES[id]}» después`} disabled={i === order.length - 1} onClick={() => move(id, 1)}>
                      →
                    </button>
                  </span>
                </header>
                {expanded && <div className="cp-tile-body">{body(id)}</div>}
              </section>
            );
          })}
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
          <button type="button" className="btn-quiet" onClick={resetOrder} disabled={busy || order.every((id, i) => id === DEFAULT_ORDER[i])}>
            Restablecer el orden
          </button>
          {attempted && Object.keys(errors).length > 0 && <span className="hint">Revisa los mosaicos marcados.</span>}
        </div>
      </form>
    );
  }

  // ---- Mission 2: the topic's questions as cards ----------------------------------------------------
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
      <p className="hint">
        Misión 2 · Elige la carta que más se parezca a ti. Así no te explico lo que ya sabes ni supongo lo que no tienes.
      </p>

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
        {attempted && Object.keys(answerErrs).length > 0 && <span className="hint">Revisa las preguntas marcadas.</span>}
      </div>
    </form>
  );
}
