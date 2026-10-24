import { useEffect, useState } from "react";
import { formatWeekly, parseDaysPerWeek, weeklyHours } from "../profileFormModel";
import { DAYS, HOURS_SLIDER, type DayId, daysFromDraft, hoursToDraft, sliderHours, toggleDay } from "./panelModel";

/**
 * "How much time do you have": seven day switches plus an hours-per-day slider.
 * It stays controlled by the two draft strings the backend model already
 * understands (`daysPerWeek` is a COUNT, `hoursPerDay` a number), and shows the
 * weekly total live. Which weekdays are lit is only a visual choice.
 */
export function RhythmTile({
  daysPerWeek,
  hoursPerDay,
  onChange,
  disabled,
  error,
}: {
  daysPerWeek: string;
  hoursPerDay: string;
  onChange: (patch: { daysPerWeek?: string; hoursPerDay?: string }) => void;
  disabled?: boolean;
  error?: string;
}) {
  const [days, setDays] = useState<DayId[]>(() => daysFromDraft(daysPerWeek));
  // An example, a reopened session or the plan on screen changes the count from outside: relight to match.
  useEffect(() => {
    const n = parseDaysPerWeek(daysPerWeek);
    if (n !== null && n !== days.length) setDays(daysFromDraft(daysPerWeek));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [daysPerWeek]);

  const weekly = weeklyHours(hoursPerDay, daysPerWeek);
  const flip = (id: DayId) => {
    const next = toggleDay(days, id);
    setDays(next);
    onChange({ daysPerWeek: String(next.length) });
  };

  return (
    <div className="rhythm-tile">
      <div className="day-switches" role="group" aria-label="Días de estudio">
        {DAYS.map((d) => {
          const on = days.includes(d.id);
          return (
            <button key={d.id} type="button" role="switch" aria-checked={on} aria-label={d.label} className={`day-switch${on ? " on" : ""}`} onClick={() => flip(d.id)} disabled={disabled}>
              {d.short}
            </button>
          );
        })}
      </div>
      <label className="pf-field">
        <span className="pf-label">Horas por día: {hoursPerDay.trim() === "" ? "–" : hoursPerDay.replace(".", ",")}</span>
        <input
          type="range"
          className="hours-slider"
          min={HOURS_SLIDER.min}
          max={HOURS_SLIDER.max}
          step={HOURS_SLIDER.step}
          value={sliderHours(hoursPerDay)}
          onChange={(e) => onChange({ hoursPerDay: hoursToDraft(Number(e.target.value)) })}
          disabled={disabled}
        />
      </label>
      {weekly !== null && !error && <p className="hint pf-total">Son {formatWeekly(weekly)}.</p>}
      {error && (
        <span className="field-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
