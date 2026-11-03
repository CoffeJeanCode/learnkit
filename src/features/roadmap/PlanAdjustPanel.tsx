import { useId, useState } from "react";
import type { PlanAdjustmentPayload, PlanEmphasis } from "../../lib/schemas";
import { EMPHASIS_OPTIONS, LIMITS, adjustDraftFrom, adjustmentFrom } from "./profileFormModel";
import { RhythmTile } from "./panel/RhythmTile";
import { WeeksDial } from "./panel/WeeksDial";

/**
 * "Ajustar el plan" with the same controls as the board that made it: the
 * weeks dial, the day switches + hours slider, where to lean (practice /
 * theory / project) as cards, and an optional short note. Only what actually
 * changed is sent, and one proposal turn answers it. Mount it with a `key`
 * that changes per proposal so it resets to the new plan's values.
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
  const toggleEmphasis = (id: PlanEmphasis) => setDraft((d) => ({ ...d, emphasis: d.emphasis === id ? "" : id }));

  return (
    <details className="adjust-panel">
      <summary>Ajustar el plan</summary>
      <div className="adjust-body">
        <div className="pf-field">
          <span className="pf-label">Semanas</span>
          <WeeksDial value={draft.weeks} onChange={(weeks) => setDraft((d) => ({ ...d, weeks }))} disabled={busy} />
          {errors.weeks && <span className="field-error">{errors.weeks}</span>}
        </div>

        <div className="pf-field">
          <span className="pf-label">Tu ritmo</span>
          <RhythmTile
            daysPerWeek={draft.daysPerWeek}
            hoursPerDay={draft.hoursPerDay}
            onChange={(patch) => setDraft((d) => ({ ...d, ...patch }))}
            disabled={busy}
            error={errors.weekly ?? errors.hoursPerDay ?? errors.daysPerWeek}
          />
        </div>

        <fieldset className="pf-field pf-fieldset" disabled={busy}>
          <legend className="pf-label">¿Hacia dónde inclino el plan?</legend>
          <div className="card-deck">
            {EMPHASIS_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                type="button"
                className={`pick-card${draft.emphasis === opt.id ? " selected" : ""}`}
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
