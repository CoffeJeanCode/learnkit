import type { PlanDraft } from "./planDraft";
import { draftProgress } from "./planDraft";

/** "2,5 h" with the learner's decimal comma. */
function hoursLabel(hours: number): string {
  return `${String(hours).replace(".", ",")} h`;
}

/**
 * The plan as it is being written: the outline appears first (title, one line per
 * week), then each week's sessions replace their skeleton as they land. Every new
 * piece fades in (`.draft-fade`), staggered by position so a burst of weeks reads as
 * a cascade rather than a flash. Nothing here is the real plan yet — it is replaced
 * by the proposal card once the turn resolves.
 */
export function PlanDraftView({ draft }: { draft: PlanDraft }) {
  const { ready, total } = draftProgress(draft);
  return (
    <div className="card plan-draft" aria-live="polite" aria-busy={ready < total}>
      <div className="card-head draft-fade">
        <h3>{draft.courseTitle}</h3>
        <span className="badge">escribiendo tu plan…</span>
      </div>
      <p className="hint draft-fade" style={{ ["--i" as string]: 1 }}>
        {draft.coreFocus}
      </p>
      <p className="hint plan-draft-progress">
        {ready === total ? "Revisando que todo encaje…" : `${ready} de ${total} semanas con sus sesiones`}
      </p>
      <ul className="dod-list milestone-list">
        {draft.weeks.map((w, i) => (
          <li key={w.week} className="draft-fade" style={{ ["--i" as string]: i + 2 }}>
            <strong>
              Semana {w.week}: {w.title}
            </strong>
            {w.weeklyGoal && (
              <blockquote className="week-goal">
                <strong>Meta semanal:</strong> {w.weeklyGoal}
              </blockquote>
            )}
            {w.sessions ? (
              <ul className="dod-list">
                {w.sessions.map((s, j) => (
                  <li key={j} className="draft-fade" style={{ ["--i" as string]: j }}>
                    <span className="draft-session-label">{s.label}</span> <span className="hint">· {hoursLabel(s.hours)}</span>
                    {s.objective && <div className="hint">{s.objective}</div>}
                  </li>
                ))}
              </ul>
            ) : (
              <div className="gen-skeleton compact" aria-hidden="true">
                <span className="sk-line" style={{ width: "70%" }} />
                <span className="sk-line" style={{ width: "55%" }} />
              </div>
            )}
          </li>
        ))}
      </ul>
      <p className="hint draft-fade" style={{ ["--i" as string]: draft.weeks.length + 2 }}>
        Proyecto final: {draft.capstoneTitle}
      </p>
    </div>
  );
}
