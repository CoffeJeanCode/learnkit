import { useRef } from "react";
import { DIAL_MAX, DIAL_MIN, DIAL_SWEEP, WEEK_NOTCHES, angleToWeeks, arcPath, clampWeeks, pointerAngle, stepWeeks, weeksToAngle } from "./panelModel";

const SIZE = 168;
const C = SIZE / 2;
const R = 62;

/**
 * The weeks as a dial: drag the knob around the arc, use the arrow keys, tap a
 * quick pick, or type the number. `value` is the draft string, so an empty or
 * invalid draft shows an idle dial instead of inventing a number.
 */
export function WeeksDial({ value, onChange, disabled }: { value: string; onChange: (weeks: string) => void; disabled?: boolean }) {
  const ref = useRef<SVGSVGElement>(null);
  const dragging = useRef(false);
  const parsed = Number(value);
  const weeks = value.trim() !== "" && Number.isInteger(parsed) && parsed >= DIAL_MIN && parsed <= DIAL_MAX ? parsed : null;
  const angle = weeksToAngle(weeks ?? DIAL_MIN);
  const knob = { x: C + R * Math.sin((angle * Math.PI) / 180), y: C - R * Math.cos((angle * Math.PI) / 180) };
  const set = (n: number) => onChange(String(clampWeeks(n)));

  const fromPointer = (e: React.PointerEvent) => {
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;
    const dx = e.clientX - (box.left + box.width / 2);
    const dy = e.clientY - (box.top + box.height / 2);
    const raw = pointerAngle(dx, dy);
    // Dragging through the dead zone at the bottom snaps to the nearer end.
    set(angleToWeeks(Math.abs(raw) > DIAL_SWEEP / 2 ? Math.sign(raw) * (DIAL_SWEEP / 2) : raw));
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const base = weeks ?? DIAL_MIN;
    const next =
      e.key === "ArrowRight" || e.key === "ArrowUp" ? stepWeeks(base, 1)
      : e.key === "ArrowLeft" || e.key === "ArrowDown" ? stepWeeks(base, -1)
      : e.key === "PageUp" ? stepWeeks(base, 4)
      : e.key === "PageDown" ? stepWeeks(base, -4)
      : e.key === "Home" ? DIAL_MIN
      : e.key === "End" ? DIAL_MAX
      : null;
    if (next === null) return;
    e.preventDefault();
    set(next);
  };

  return (
    <div className="weeks-dial">
      <svg
        ref={ref}
        className="dial-svg"
        width={SIZE}
        height={SIZE}
        viewBox={`0 0 ${SIZE} ${SIZE}`}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        aria-label="Semanas"
        aria-valuemin={DIAL_MIN}
        aria-valuemax={DIAL_MAX}
        aria-valuenow={weeks ?? undefined}
        aria-valuetext={weeks === null ? "sin elegir" : `${weeks} ${weeks === 1 ? "semana" : "semanas"}`}
        aria-disabled={disabled}
        onKeyDown={disabled ? undefined : onKeyDown}
        onPointerDown={(e) => {
          if (disabled) return;
          dragging.current = true;
          e.currentTarget.setPointerCapture(e.pointerId);
          fromPointer(e);
        }}
        onPointerMove={(e) => dragging.current && fromPointer(e)}
        onPointerUp={() => (dragging.current = false)}
        onPointerCancel={() => (dragging.current = false)}
      >
        <path className="dial-track" d={arcPath(C, C, R, -DIAL_SWEEP / 2, DIAL_SWEEP / 2)} />
        {weeks !== null && weeks > DIAL_MIN && <path className="dial-fill" d={arcPath(C, C, R, -DIAL_SWEEP / 2, angle)} />}
        <circle className="dial-knob" cx={knob.x} cy={knob.y} r={11} />
        <text className="dial-value" x={C} y={C + 6} textAnchor="middle">
          {weeks ?? "–"}
        </text>
        <text className="dial-unit" x={C} y={C + 28} textAnchor="middle">
          {weeks === 1 ? "semana" : "semanas"}
        </text>
      </svg>
      <div className="chip-row dial-notches">
        {WEEK_NOTCHES.map((n) => (
          <button key={n} type="button" className={`chip-choice${weeks === n ? " selected" : ""}`} aria-pressed={weeks === n} onClick={() => set(n)} disabled={disabled}>
            {n}
          </button>
        ))}
      </div>
    </div>
  );
}
