import { useId, useState } from "react";
import type { PlanAdjustmentPayload, PlanEmphasis } from "../../lib/schemas";
import { EMPHASIS_OPTIONS, LIMITS, adjustDraftFrom, adjustmentFrom, formatWeekly, weeklyHours } from "./profileFormModel";

/**
 * "Ajustar el plan" with controls instead of a negotiation chat: weeks, hours,
 * where to lean (practice / theory / project) and an optional short note. Only
 * what actually changed is sent, and one proposal turn answers it. Mount it
 * with a `key` that changes per proposal so it resets to the new plan's values.
 */
export function PlanAdjustPanel({
  current,
  busy,
  onAdjust,
}: {
  current: { weeks: number; hours: number };
  busy: boolean;
  onAdjust: (adjustment: PlanAdjustmentPayload) => void;
}) {
  const uid = useId();
  const [draft, setDraft] = useState(() => adjustDraftFrom(current));
  const { payload, errors } = adjustmentFrom(draft, current);
  const weekly = weeklyHours(draft.hoursPerDay, draft.daysPerWeek);
  const toggleEmphasis = (id: PlanEmphasis) => setDraft((d) => ({ ...d, emphasis: d.emphasis === id ? "" : id }));

  return (
    <details className="adjust-panel">
      <summary>Ajustar el plan</summary>
      <div className="adjust-body">
        <div className="pf-row pf-row-3">
          <label className="pf-field">
            <span className="pf-label">Semanas</span>
            <input
              inputMode="numeric"
              value={draft.weeks}
              onChange={(e) => setDraft((d) => ({ ...d, weeks: e.target.value }))}
              disabled={busy}
            />
            {errors.weeks && <span className="field-error">{errors.weeks}</span>}
          </label>
          <label className="pf-field">
            <span className="pf-label">Horas por día</span>
            <input
              inputMode="decimal"
              value={draft.hoursPerDay}
              onChange={(e) => setDraft((d) => ({ ...d, hoursPerDay: e.target.value }))}
              disabled={busy}
            />
            {errors.hoursPerDay && <span className="field-error">{errors.hoursPerDay}</span>}
          </label>
          <label className="pf-field">
            <span className="pf-label">Días por semana</span>
            <input
              inputMode="numeric"
              value={draft.daysPerWeek}
              onChange={(e) => setDraft((d) => ({ ...d, daysPerWeek: e.target.value }))}
              disabled={busy}
            />
            {errors.daysPerWeek && <span className="field-error">{errors.daysPerWeek}</span>}
          </label>
        </div>
        {weekly !== null && !errors.weekly && <p className="hint">Son {formatWeekly(weekly)}.</p>}
        {errors.weekly && <span className="field-error">{errors.weekly}</span>}

        <fieldset className="pf-field pf-fieldset" disabled={busy}>
          <legend className="pf-label">¿Hacia dónde inclino el plan?</legend>
          <div className="chip-row">
            {EMPHASIS_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                type="button"
                className={`chip-choice${draft.emphasis === opt.id ? " selected" : ""}`}
                aria-pressed={draft.emphasis === opt.id}
                onClick={() => toggleEmphasis(opt.id)}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </fieldset>

        <label className="pf-field" htmlFor={`${uid}-note`}>
          <span className="pf-label">Algo más que deba saber (opcional)</span>
          <textarea
            id={`${uid}-note`}
            rows={2}
            value={draft.note}
            onChange={(e) => setDraft((d) => ({ ...d, note: e.target.value }))}
            placeholder="Ej.: tengo un examen en junio"
            disabled={busy}
          />
          <span className="hint">
            {draft.note.length}/{LIMITS.noteMax}
          </span>
          {errors.note && <span className="field-error">{errors.note}</span>}
        </label>

        <div className="row">
          <button className="btn-quiet" type="button" disabled={busy || payload === null} onClick={() => payload && onAdjust(payload)}>
            {busy ? "Ajustando…" : "Pedir este ajuste"}
          </button>
          {payload === null && !busy && <span className="hint">Cambia algo para poder pedirlo.</span>}
        </div>
      </div>
    </details>
  );
}
