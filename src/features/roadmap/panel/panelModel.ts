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

// --- Tiles -----------------------------------------------------------------------------------

export type TileId = "topic" | "goal" | "weeks" | "rhythm";
export const DEFAULT_ORDER: readonly TileId[] = ["topic", "goal", "weeks", "rhythm"];
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
  const total = DEFAULT_ORDER.length;
  const done = DEFAULT_ORDER.filter((id) => tileStatus(id, draft, false) === "ready").length;
  return { done, total };
}

/** Moves one tile a step; a no-op at either end. */
export function moveTile(order: readonly TileId[], id: TileId, dir: -1 | 1): TileId[] {
  const from = order.indexOf(id);
  const to = from + dir;
  if (from === -1 || to < 0 || to >= order.length) return [...order];
  const next = [...order];
  [next[from], next[to]] = [next[to], next[from]];
  return next;
}

// --- Persisted layout ----------------------------------------------------------------------------

export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const ORDER_STORAGE_KEY = "learnkit.controlPanel.order";

/** The saved tile order, or the default when nothing valid is stored (a missing tile,
 *  an unknown one, a repeat, bad JSON or a store that throws all fall back). */
export function loadOrder(store: KeyValueStore | null): TileId[] {
  try {
    const raw = store?.getItem(ORDER_STORAGE_KEY);
    if (!raw) return [...DEFAULT_ORDER];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [...DEFAULT_ORDER];
    const valid = parsed.length === DEFAULT_ORDER.length && DEFAULT_ORDER.every((id) => parsed.filter((p) => p === id).length === 1);
    return valid ? (parsed as TileId[]) : [...DEFAULT_ORDER];
  } catch {
    return [...DEFAULT_ORDER];
  }
}

/** Best effort: the layout still works for this session when storage is unavailable. */
export function saveOrder(store: KeyValueStore | null, order: readonly TileId[]): void {
  try {
    store?.setItem(ORDER_STORAGE_KEY, JSON.stringify(order));
  } catch {
    /* private window, blocked site data, quota: ignore */
  }
}

export function browserStore(): KeyValueStore | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}
