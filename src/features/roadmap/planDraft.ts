import type { PlanPartEvent } from "../../lib/schemas";

/** One week of the plan while it is being written. `sessions` stays `null` until
 *  that week's sessions arrive: the UI shows a skeleton for it meanwhile. */
export interface DraftWeek {
  week: number;
  title: string;
  weeklyGoal: string | null;
  sessions: { label: string; hours: number; objective: string | null }[] | null;
}

/** The syllabus as far as it has been written. */
export interface PlanDraft {
  courseTitle: string;
  coreFocus: string;
  capstoneTitle: string;
  weeks: DraftWeek[];
}

/** Folds one progress event into the draft.
 *  - `started` clears it (a new attempt: whatever was shown is stale);
 *  - `outline` creates it, every week still waiting for its sessions;
 *  - `week` fills that week's sessions — ignored when there is no outline yet
 *    or the week is unknown, so a late or duplicated event can never invent a
 *    plan. Events of another session are ignored. */
export function applyPlanPart(draft: PlanDraft | null, event: PlanPartEvent, sessionId: string): PlanDraft | null {
  if (event.sessionId !== sessionId) return draft;
  switch (event.kind) {
    case "started":
      return null;
    case "outline":
      return {
        courseTitle: event.courseTitle,
        coreFocus: event.coreFocus,
        capstoneTitle: event.capstoneTitle,
        weeks: [...event.weeks]
          .sort((a, b) => a.week - b.week)
          .map((w) => ({ week: w.week, title: w.title, weeklyGoal: w.weeklyGoal ?? null, sessions: null })),
      };
    case "week": {
      if (!draft || !draft.weeks.some((w) => w.week === event.week)) return draft;
      const sessions = event.sessions.map((s) => ({ label: s.label, hours: s.hours, objective: s.objective ?? null }));
      return { ...draft, weeks: draft.weeks.map((w) => (w.week === event.week ? { ...w, sessions } : w)) };
    }
  }
}

/** "3 de 8 semanas listas" — how far along the draft is. */
export function draftProgress(draft: PlanDraft): { ready: number; total: number } {
  return { ready: draft.weeks.filter((w) => w.sessions !== null).length, total: draft.weeks.length };
}
