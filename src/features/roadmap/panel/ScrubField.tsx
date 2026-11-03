import { useRef, useState } from "react";
import { markPosition, scrub } from "./panelModel";

/**
 * A number you drag sideways, like a parameter field in a node editor: press
 * and drag left/right to change it (Shift = ten times finer), double-click or
 * Enter to type it instead, arrow keys to nudge. `value` is the draft string
 * (empty stays empty until the first change), so it plugs into the same
 * string drafts as every other input. The thin bar shows where the value sits
 * in the scrubbable range.
 */
export function ScrubField({
  label,
  value,
  fallback,
  min,
  max,
  step,
  unit,
  marks,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  /** What a drag starts from while the draft is empty or invalid. */
  fallback: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  /** Legend stops along the track (like the scale of a MIDI fader): tap one to jump there. */
  marks?: readonly number[];
  onChange: (next: string) => void;
  disabled?: boolean;
}) {
  const parsed = Number(value.replace(",", "."));
  const current = value.trim() !== "" && Number.isFinite(parsed) ? parsed : fallback;
  const shown = value.trim() === "" ? "–" : value.replace(".", ",");
  const press = useRef<{ x: number; start: number; moved: boolean } | null>(null);
  const [typing, setTyping] = useState(false);
  const commit = (n: number) => onChange(String(n));

  const nudge = (dir: 1 | -1, fine: boolean) => {
    const unitStep = fine ? step / 10 : step;
    commit(Math.min(max, Math.max(min, Math.round((current + dir * unitStep) * 1000) / 1000)));
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") nudge(1, e.shiftKey);
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") nudge(-1, e.shiftKey);
    else if (e.key === "Home") commit(min);
    else if (e.key === "End") commit(max);
    else if (e.key === "Enter") setTyping(true);
    else return;
    e.preventDefault();
  };

  if (typing) {
    return (
      <label className="scrub scrub-typing">
        <span className="pf-label">{label}</span>
        <input
          autoFocus
          defaultValue={value}
          inputMode="decimal"
          aria-label={label}
          onBlur={(e) => {
            onChange(e.target.value.trim());
            setTyping(false);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") setTyping(false);
          }}
        />
      </label>
    );
  }

  return (
    <div
      className={`scrub${disabled ? " is-disabled" : ""}`}
      role="spinbutton"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={current}
      aria-valuetext={`${shown} ${unit}`}
      aria-disabled={disabled}
      title="Arrastra de lado para cambiarlo · Shift = más fino · doble clic para escribir"
      onKeyDown={onKeyDown}
      onDoubleClick={() => !disabled && setTyping(true)}
      onPointerDown={(e) => {
        if (disabled || e.button !== 0) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        press.current = { x: e.clientX, start: current, moved: false };
      }}
      onPointerMove={(e) => {
        const p = press.current;
        if (!p) return;
        const dx = e.clientX - p.x;
        if (Math.abs(dx) < 3 && !p.moved) return;
        p.moved = true;
        commit(scrub(p.start, dx, { step, min, max, pxPerStep: 12, fine: e.shiftKey }));
      }}
      onPointerUp={() => (press.current = null)}
      onPointerCancel={() => (press.current = null)}
    >
      <span className="pf-label">{label}</span>
      <span className="scrub-value">
        {shown} <small>{unit}</small>
      </span>
      <span className="scrub-track" aria-hidden="true">
        <span className="scrub-fill" style={{ width: `${markPosition(current, min, max)}%` }} />
      </span>
      {marks && marks.length > 0 && (
        <span className="scrub-marks">
          {marks.map((m) => (
            <button
              key={m}
              type="button"
              tabIndex={-1}
              className={`scrub-mark${Math.abs(current - m) < 1e-6 ? " is-on" : ""}`}
              style={{ left: `${markPosition(m, min, max)}%` }}
              aria-label={`Poner ${String(m).replace(".", ",")} ${unit}`}
              disabled={disabled}
              // A tap on a legend must not start a scrub of the field underneath.
              onPointerDown={(e) => e.stopPropagation()}
              onDoubleClick={(e) => e.stopPropagation()}
              onClick={() => commit(m)}
            >
              <i aria-hidden="true" />
              <span>{String(m).replace(".", ",")}</span>
            </button>
          ))}
        </span>
      )}
    </div>
  );
}
