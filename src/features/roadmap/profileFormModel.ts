import {
  type ContextAnswer,
  type ContextQuestion,
  type ContextQuestionSet,
  type PlanAdjustmentPayload,
  type PlanEmphasis,
  type ProfileBasics,
  UNKNOWN_ANSWER,
} from "../../lib/schemas";

/** The same ranges the backend enforces (`profile_grounding_violations` and the
 *  intake-answer validation), so the form refuses what the server would. The
 *  backend only knows HOURS PER WEEK; the form asks per day (easier to estimate)
 *  and converts, so the per-day limits below just keep the inputs sane. */
export const LIMITS = {
  weeksMin: 1,
  weeksMax: 52,
  /** Weekly total the backend accepts. */
  hoursMin: 0.5,
  hoursMax: 80,
  hoursPerDayMin: 0.25,
  hoursPerDayMax: 24,
  daysMin: 1,
  daysMax: 7,
  noteMax: 300,
  textAnswerMax: 200,
} as const;

/** Days per week assumed until the learner changes it. */
export const DEFAULT_DAYS_PER_WEEK = "5";

export const EMPHASIS_OPTIONS: { id: PlanEmphasis; label: string }[] = [
  { id: "more_practice", label: "Más práctica" },
  { id: "more_theory", label: "Más teoría" },
  { id: "more_projects", label: "Más proyecto" },
];

// --- Time: hours per day x days per week = hours per week -----------------------------

function parseNumber(raw: string): number | null {
  const n = Number(raw.replace(",", "."));
  return raw.trim() !== "" && Number.isFinite(n) ? n : null;
}

/** Hours per day, or null when empty/out of range. */
export function parseHoursPerDay(raw: string): number | null {
  const n = parseNumber(raw);
  return n !== null && n >= LIMITS.hoursPerDayMin && n <= LIMITS.hoursPerDayMax ? n : null;
}

/** Whole days per week 1..7, or null. */
export function parseDaysPerWeek(raw: string): number | null {
  const n = parseNumber(raw);
  return n !== null && Number.isInteger(n) && n >= LIMITS.daysMin && n <= LIMITS.daysMax ? n : null;
}

/** Hours per week from the two inputs, rounded to 2 decimals (so 0.1 x 3 is 0.3,
 *  not 0.30000000000000004), or null while either input is invalid. */
export function weeklyHours(hoursPerDay: string, daysPerWeek: string): number | null {
  const h = parseHoursPerDay(hoursPerDay);
  const d = parseDaysPerWeek(daysPerWeek);
  return h === null || d === null ? null : Math.round(h * d * 100) / 100;
}

/** Splits a weekly total back into per-day inputs — for showing what the server
 *  kept. Prefers 5 days, then other day counts, taking the first that gives a
 *  clean per-day value; the product is always exactly the stored weekly total. */
export function splitWeekly(weekly: number): { hoursPerDay: string; daysPerWeek: string } {
  for (const days of [5, 7, 6, 4, 3, 2, 1]) {
    const perDay = weekly / days;
    if (perDay >= LIMITS.hoursPerDayMin && Math.abs(perDay * 100 - Math.round(perDay * 100)) < 1e-9) {
      return { hoursPerDay: String(Math.round(perDay * 100) / 100), daysPerWeek: String(days) };
    }
  }
  return { hoursPerDay: String(weekly), daysPerWeek: "1" };
}

/** "5 h por semana" / "10,5 h por semana", with the learner's decimal comma. */
export function formatWeekly(hours: number): string {
  return `${String(hours).replace(".", ",")} h por semana`;
}

// --- Step 1: topic, goal, time ----------------------------------------------------

/** What the learner has typed in step 1. Numbers stay strings while editing. */
export interface BasicsDraft {
  topic: string;
  targetGoal: string;
  weeks: string;
  hoursPerDay: string;
  daysPerWeek: string;
}

export function emptyBasics(): BasicsDraft {
  return { topic: "", targetGoal: "", weeks: "", hoursPerDay: "", daysPerWeek: DEFAULT_DAYS_PER_WEEK };
}

/** Examples that fill step 1 (not a chat message). No topic is special: a
 *  biology one sits next to a camera one on purpose. */
export const EXAMPLES: { label: string; tone: "red" | "green" | "blue" | "gold"; draft: BasicsDraft }[] = [
  { label: "Mutaciones genéticas", tone: "green", draft: { topic: "Mutaciones genéticas", targetGoal: "Explicar cómo ocurre una mutación y qué efectos tiene", weeks: "6", hoursPerDay: "1", daysPerWeek: "3" } },
  { label: "Guitarra desde cero", tone: "red", draft: { topic: "Guitarra", targetGoal: "Tocar tres canciones completas", weeks: "12", hoursPerDay: "0.5", daysPerWeek: "5" } },
  { label: "Linux y la terminal", tone: "blue", draft: { topic: "Linux y terminal", targetGoal: "Manejar mi propia computadora desde la terminal", weeks: "8", hoursPerDay: "1", daysPerWeek: "4" } },
  { label: "Fotografía manual", tone: "gold", draft: { topic: "Fotografía", targetGoal: "Tomar fotos bien expuestas en modo manual", weeks: "8", hoursPerDay: "0.5", daysPerWeek: "6" } },
];

function parseWeeks(raw: string): number | null {
  const n = parseNumber(raw);
  return n !== null && Number.isInteger(n) && n >= LIMITS.weeksMin && n <= LIMITS.weeksMax ? n : null;
}

/** The message for the weekly total when it falls outside what the backend accepts. */
function weeklyRangeError(weekly: number): string | null {
  return weekly >= LIMITS.hoursMin && weekly <= LIMITS.hoursMax
    ? null
    : `Eso suma ${String(weekly).replace(".", ",")} h por semana; tiene que estar entre ${String(LIMITS.hoursMin).replace(".", ",")} y ${LIMITS.hoursMax}.`;
}

/** Field -> message for what is wrong with step 1 ({} when valid). Keys: topic,
 *  targetGoal, weeks, hoursPerDay, daysPerWeek, weekly (the product out of range). */
export function basicsErrors(draft: BasicsDraft): Record<string, string> {
  const errors: Record<string, string> = {};
  if (!draft.topic.trim()) errors.topic = "Dime qué quieres aprender.";
  if (!draft.targetGoal.trim()) errors.targetGoal = "Dime qué quieres lograr con eso.";
  if (parseWeeks(draft.weeks) === null) errors.weeks = `Entre ${LIMITS.weeksMin} y ${LIMITS.weeksMax} semanas, en números enteros.`;
  if (parseHoursPerDay(draft.hoursPerDay) === null) errors.hoursPerDay = `Entre ${String(LIMITS.hoursPerDayMin).replace(".", ",")} y ${LIMITS.hoursPerDayMax} horas por día.`;
  if (parseDaysPerWeek(draft.daysPerWeek) === null) errors.daysPerWeek = `De ${LIMITS.daysMin} a ${LIMITS.daysMax} días por semana.`;
  const weekly = weeklyHours(draft.hoursPerDay, draft.daysPerWeek);
  if (weekly !== null) {
    const range = weeklyRangeError(weekly);
    if (range) errors.weekly = range;
  }
  return errors;
}

/** The request for step 1, or `null` while the draft is invalid. */
export function toBasicsPayload(draft: BasicsDraft): ProfileBasics | null {
  if (Object.keys(basicsErrors(draft)).length > 0) return null;
  return {
    topic: draft.topic.trim(),
    targetGoal: draft.targetGoal.trim(),
    timeframeWeeks: parseWeeks(draft.weeks)!,
    weeklyCommitmentHours: weeklyHours(draft.hoursPerDay, draft.daysPerWeek)!,
  };
}

/** Rebuilds the editable draft from what the server kept (resuming at step 2 and
 *  going back to step 1 must show what was typed). */
export function basicsToDraft(basics: ProfileBasics): BasicsDraft {
  return { topic: basics.topic, targetGoal: basics.targetGoal, weeks: String(basics.timeframeWeeks), ...splitWeekly(basics.weeklyCommitmentHours) };
}

/** The one-line "you said" bubble shown after step 2 — same wording as the turn
 *  the backend logs, so a reopened session reads the same. The level is not in
 *  it: it is deduced from the answers, not something the learner said. */
export function profileSummaryText(b: ProfileBasics): string {
  return `Tema: ${b.topic} · Meta: ${b.targetGoal} · ${b.timeframeWeeks} semanas · ${b.weeklyCommitmentHours} h/semana`;
}

// --- Step 2: the questions generated for this topic ---------------------------------

/** question id -> answer. */
export type AnswersDraft = Record<string, string>;

export { UNKNOWN_ANSWER };

/** The questions being ASKED right now, in screen order: every top-level one, and
 *  right after it its follow-ups — but only once it was answered "No lo sé".
 *  Not knowing opens simpler questions instead of forcing a guess. */
export function visibleQuestions(set: ContextQuestionSet, answers: AnswersDraft): ContextQuestion[] {
  return set.questions.flatMap((q) =>
    (answers[q.id] ?? "").trim() === UNKNOWN_ANSWER ? [q, ...q.followUps] : [q],
  );
}

/** Field -> message for what is wrong with the answers. Keys: `q:<id>`. Every
 *  visible choice must be one of its options or "No lo sé"; free text is optional but capped. */
export function answerErrors(set: ContextQuestionSet, answers: AnswersDraft): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const q of visibleQuestions(set, answers)) {
    const value = (answers[q.id] ?? "").trim();
    if (q.kind === "choice" && value !== UNKNOWN_ANSWER && !q.options.some((o) => o.label === value)) errors[`q:${q.id}`] = "Elige una opción.";
    if (q.kind === "text" && value.length > LIMITS.textAnswerMax) errors[`q:${q.id}`] = `Máximo ${LIMITS.textAnswerMax} caracteres.`;
  }
  return errors;
}

/** The answers for the backend, or `null` while any is invalid. Only the
 *  questions being asked are sent (a follow-up whose parent changed its answer is
 *  dropped, the server would refuse it); blank free text is dropped. */
export function toAnswers(set: ContextQuestionSet, answers: AnswersDraft): ContextAnswer[] | null {
  if (Object.keys(answerErrors(set, answers)).length > 0) return null;
  return visibleQuestions(set, answers)
    .map((q) => ({ questionId: q.id, answer: (answers[q.id] ?? "").trim() }))
    .filter((a) => a.answer !== "");
}

// --- Plan adjustment ------------------------------------------------------------------

export interface AdjustDraft {
  weeks: string;
  hoursPerDay: string;
  daysPerWeek: string;
  emphasis: PlanEmphasis | "";
  note: string;
}

/** `current.hours` is the plan's hours PER WEEK, split back into per-day inputs. */
export function adjustDraftFrom(current: { weeks: number; hours: number }): AdjustDraft {
  return { weeks: String(current.weeks), ...splitWeekly(current.hours), emphasis: "", note: "" };
}

/** What changed relative to the proposal on screen, or why it cannot be sent.
 *  `payload` is `null` when nothing changed (the button stays disabled). */
export function adjustmentFrom(
  draft: AdjustDraft,
  current: { weeks: number; hours: number },
): { payload: PlanAdjustmentPayload | null; errors: Record<string, string> } {
  const errors: Record<string, string> = {};
  const weeks = parseWeeks(draft.weeks);
  if (weeks === null) errors.weeks = `Entre ${LIMITS.weeksMin} y ${LIMITS.weeksMax} semanas.`;
  if (parseHoursPerDay(draft.hoursPerDay) === null) errors.hoursPerDay = `Entre ${String(LIMITS.hoursPerDayMin).replace(".", ",")} y ${LIMITS.hoursPerDayMax} horas por día.`;
  if (parseDaysPerWeek(draft.daysPerWeek) === null) errors.daysPerWeek = `De ${LIMITS.daysMin} a ${LIMITS.daysMax} días.`;
  const weekly = weeklyHours(draft.hoursPerDay, draft.daysPerWeek);
  if (weekly !== null) {
    const range = weeklyRangeError(weekly);
    if (range) errors.weekly = range;
  }
  if (draft.note.length > LIMITS.noteMax) errors.note = `Máximo ${LIMITS.noteMax} caracteres.`;
  if (Object.keys(errors).length > 0 || weeks === null || weekly === null) return { payload: null, errors };

  const payload: PlanAdjustmentPayload = {
    timeframeWeeks: weeks !== current.weeks ? weeks : null,
    weeklyCommitmentHours: weekly !== current.hours ? weekly : null,
    emphasis: draft.emphasis === "" ? null : draft.emphasis,
    note: draft.note.trim() === "" ? null : draft.note.trim(),
  };
  const changed = payload.timeframeWeeks !== null || payload.weeklyCommitmentHours !== null || payload.emphasis !== null || payload.note !== null;
  return { payload: changed ? payload : null, errors };
}
