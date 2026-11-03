import { type BasicsDraft, LIMITS, basicsErrors, parseDaysPerWeek } from "../profileFormModel";

// --- Weeks dial ---------------------------------------------------------------------------
// A 270° dial: 0 is straight up, the knob sweeps from -135° (min) to +135° (max).

export const DIAL_MIN = LIMITS.weeksMin;
export const DIAL_MAX = LIMITS.weeksMax;
export const DIAL_SWEEP = 270;
/** Quick picks drawn under the dial. */
export const WEEK_NOTCHES = [4, 8, 12, 24] as const;

export function clampWeeks(n: number): number {
  return Math.min(DIAL_MAX, Math.max(DIAL_MIN, Math.round(n)));
}

/** Knob angle (-135..135) for a number of weeks. */
export function weeksToAngle(weeks: number): number {
  const t = (clampWeeks(weeks) - DIAL_MIN) / (DIAL_MAX - DIAL_MIN);
  return -DIAL_SWEEP / 2 + t * DIAL_SWEEP;
}

/** Weeks for a knob angle; anything past either end snaps to that end. */
export function angleToWeeks(angle: number): number {
  const clamped = Math.min(DIAL_SWEEP / 2, Math.max(-DIAL_SWEEP / 2, angle));
  const t = (clamped + DIAL_SWEEP / 2) / DIAL_SWEEP;
  return clampWeeks(DIAL_MIN + t * (DIAL_MAX - DIAL_MIN));
}

/** Angle (degrees, 0 = up, clockwise, in -180..180) of a pointer `dx`,`dy` away from the dial centre. The
 *  dead zone at the bottom snaps to the nearer end, so dragging "through" it never jumps across the dial. */
export function pointerAngle(dx: number, dy: number): number {
  return (Math.atan2(dx, -dy) * 180) / Math.PI;
}

/** Keyboard step: arrows move 1, page keys move 4. Never leaves 1..52. */
export function stepWeeks(weeks: number, delta: number): number {
  return clampWeeks(weeks + delta);
}

/** SVG arc from `fromDeg` to `toDeg` on a circle (0 = up, clockwise). */
export function arcPath(cx: number, cy: number, r: number, fromDeg: number, toDeg: number): string {
  const point = (deg: number) => {
    const rad = (deg * Math.PI) / 180;
    return [cx + r * Math.sin(rad), cy - r * Math.cos(rad)] as const;
  };
  const [x1, y1] = point(fromDeg);
  const [x2, y2] = point(toDeg);
  const large = toDeg - fromDeg > 180 ? 1 : 0;
  return `M ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 ${large} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
}

// --- Day switches --------------------------------------------------------------------------

export const DAYS = [
  { id: "mon", short: "L", label: "Lunes" },
  { id: "tue", short: "M", label: "Martes" },
  { id: "wed", short: "X", label: "Miércoles" },
  { id: "thu", short: "J", label: "Jueves" },
  { id: "fri", short: "V", label: "Viernes" },
  { id: "sat", short: "S", label: "Sábado" },
  { id: "sun", short: "D", label: "Domingo" },
] as const;

export type DayId = (typeof DAYS)[number]["id"];

/** Which days to light up for a count when the learner has not picked: the
 *  work week first, spread out (3 = L-X-V), then the weekend. */
const PREFERRED: Record<number, DayId[]> = {
  1: ["mon"],
  2: ["mon", "thu"],
  3: ["mon", "wed", "fri"],
  4: ["mon", "tue", "thu", "fri"],
  5: ["mon", "tue", "wed", "thu", "fri"],
  6: ["mon", "tue", "wed", "thu", "fri", "sat"],
  7: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
};

export function daysFromCount(count: number): DayId[] {
  return [...(PREFERRED[count] ?? PREFERRED[5])];
}

/** The lit days for the draft's `daysPerWeek` string (the 5-day week if it is not a valid count). */
export function daysFromDraft(daysPerWeek: string): DayId[] {
  const n = parseDaysPerWeek(daysPerWeek);
  return n === null ? daysFromCount(5) : daysFromCount(n);
}

/** Flips one day. The last lit day cannot be switched off: a plan needs at least one. */
export function toggleDay(days: readonly DayId[], id: DayId): DayId[] {
  if (days.includes(id)) return days.length === 1 ? [...days] : days.filter((d) => d !== id);
  return DAYS.map((d) => d.id).filter((d) => d === id || days.includes(d));
}

// --- The week as a channel ------------------------------------------------------------------
// A CHOP-style view of the schedule: one sample per weekday (hours studied that day), so the
// learner SEES the shape of their week, and the sum is the weekly total.

/** Hours studied on each weekday, Monday to Sunday: `hoursPerDay` on lit days, 0 on the rest. */
export function weekSamples(days: readonly DayId[], hoursPerDay: number): number[] {
  return DAYS.map((d) => (days.includes(d.id) ? hoursPerDay : 0));
}

/** Full scale for a scope showing `peak`: the next of 1, 2, 4, 8, 16, 24 h, so the trace fills the
 *  view (a scope autoscales) while the scale stays a round number the learner can read. */
export function niceMax(peak: number): number {
  return [1, 2, 4, 8, 16, 24].find((m) => peak <= m) ?? 24;
}

/** The trace of a stepped channel (each sample holds until the next) as an SVG path in a `w` x `h` box.
 *  `max` is the full-scale value; a flat-zero channel sits on the baseline. */
export function stepPath(samples: readonly number[], w: number, h: number, max: number): string {
  if (samples.length === 0 || max <= 0) return "";
  const f = (n: number) => Math.round(n * 10) / 10;
  const y = (v: number) => f(h - (Math.min(max, Math.max(0, v)) / max) * h);
  const slot = w / samples.length;
  let d = `M 0 ${y(samples[0]!)}`;
  samples.forEach((v, i) => {
    d += ` L ${f(i * slot)} ${y(v)} L ${f((i + 1) * slot)} ${y(v)}`;
  });
  return d;
}

// --- Hours per day slider --------------------------------------------------------------------

export const HOURS_SLIDER = { min: 0.25, max: 8, step: 0.25 } as const;

/** The slider's value for the draft string (1 h while empty or invalid; the slider stops at 8 h). */
export function sliderHours(raw: string): number {
  const n = Number(raw.replace(",", "."));
  if (raw.trim() === "" || !Number.isFinite(n) || n <= 0) return 1;
  return Math.min(HOURS_SLIDER.max, Math.max(HOURS_SLIDER.min, n));
}

/** Back to the draft's string form, with the learner's decimal comma dropped (the model accepts both). */
export function hoursToDraft(hours: number): string {
  return String(Math.round(hours * 100) / 100);
}

/** Where a value sits along a track, as a 0-100 percentage (clamped to the track). */
export function markPosition(value: number, min: number, max: number): number {
  if (max <= min) return 0;
  return Math.round(Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100)) * 100) / 100;
}

/** Legend stops for the hours-per-day field: the values people actually pick. */
export const HOURS_MARKS = [0.5, 1, 2, 3, 4, 6, 8] as const;

/** Click-and-drag a number sideways to change it, like a node editor's parameter field. `dx` is the
 *  pointer travel in px; every `pxPerStep` px moves one `step`; `fine` (Shift) moves ten times slower.
 *  The result is snapped to the step grid and clamped. */
export function scrub(start: number, dx: number, opts: { step: number; min: number; max: number; pxPerStep?: number; fine?: boolean }): number {
  const unit = opts.fine ? opts.step / 10 : opts.step;
  const raw = start + (dx / (opts.pxPerStep ?? 10)) * unit;
  const snapped = Math.round(raw / unit) * unit;
  return Math.min(opts.max, Math.max(opts.min, Math.round(snapped * 1000) / 1000));
}

// --- Tiles -----------------------------------------------------------------------------------

export type TileId = "topic" | "goal" | "weeks" | "rhythm";
export const TILE_ORDER: readonly TileId[] = ["topic", "goal", "weeks", "rhythm"];
export const TILE_TITLES: Record<TileId, string> = {
  topic: "Qué aprender",
  goal: "Qué lograr",
  weeks: "Semanas",
  rhythm: "Tu ritmo",
};

export type TileStatus = "empty" | "ready" | "error";

const TILE_FIELDS: Record<TileId, string[]> = {
  topic: ["topic"],
  goal: ["targetGoal"],
  weeks: ["weeks"],
  rhythm: ["hoursPerDay", "daysPerWeek", "weekly"],
};

/** `ready` once its fields pass the same checks the backend runs; before the first
 *  attempt a pending tile is `empty` (calm), after it `error` (red). */
export function tileStatus(id: TileId, draft: BasicsDraft, attempted: boolean): TileStatus {
  const errors = basicsErrors(draft);
  const bad = TILE_FIELDS[id].some((key) => errors[key]);
  if (!bad) return "ready";
  // The weekly total is the live check: it is shown without waiting for an attempt.
  const live = id === "rhythm" && Boolean(errors.weekly);
  return attempted || live ? "error" : "empty";
}

export function missionProgress(draft: BasicsDraft): { done: number; total: number } {
  const total = TILE_ORDER.length;
  const done = TILE_ORDER.filter((id) => tileStatus(id, draft, false) === "ready").length;
  return { done, total };
}
