import { useEffect, useState } from "react";

/** The staged copy of the grading wait. Grading a free-text gate is a model
 *  round trip, so the first second is normal and the tenth is not; saying so
 *  is what keeps a slow call from reading as a dead button. */
const STAGES = [
  { afterMs: 0, label: "Calificando…" },
  { afterMs: 4000, label: "Revisando criterios…" },
  { afterMs: 12000, label: "Esto está tardando más de lo normal…" },
];

function useStageLabel(busy: boolean): string {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!busy) {
      setElapsed(0);
      return;
    }
    const startedAt = Date.now();
    const id = window.setInterval(() => setElapsed(Date.now() - startedAt), 1000);
    return () => window.clearInterval(id);
  }, [busy]);
  const stage = STAGES.filter((s) => elapsed >= s.afterMs).pop();
  return stage?.label ?? STAGES[0].label;
}

/** Visible progress for a gate that is being graded: an indeterminate bar plus
 *  the live stage copy. `aria-live` is a separate node on purpose — the button
 *  label changes too often to be a reliable announcement target. */
export function GateProgress({ busy, label }: { busy: boolean; label?: string }) {
  const stage = useStageLabel(busy);
  if (!busy) return null;
  return (
    <div className="gate-progress" role="status" aria-live="polite">
      <span className="gate-progress-bar" role="progressbar" aria-label={label ?? "Calificando"} />
      <span className="gate-progress-text">{stage}</span>
    </div>
  );
}

/** The submit action of a graded gate. Three states that used to collapse into
 *  one disabled button:
 *  - enabled: the student can submit;
 *  - disabled: it says WHY (`disabledReason`) instead of going grey silently;
 *  - busy: spinner + staged copy + progress bar, never a dead control. */
export function GateSubmitButton({
  label,
  busy,
  disabled,
  disabledReason,
  onClick,
  className = "btn-primary",
}: {
  label: string;
  busy: boolean;
  disabled?: boolean;
  /** Shown under the button whenever it is disabled and not busy. */
  disabledReason?: string | null;
  onClick: () => void;
  className?: string;
}) {
  const stage = useStageLabel(busy);
  const isDisabled = busy || disabled === true;
  return (
    <div className="gate-submit">
      <button
        type="button"
        className={`${className} gate-submit-btn${busy ? " is-busy" : ""}`.trim()}
        onClick={onClick}
        disabled={isDisabled}
        aria-busy={busy}
      >
        {busy && <span className="gate-spinner" aria-hidden="true" />}
        {busy ? stage : label}
      </button>
      {busy && (
        <span className="gate-progress-bar gate-progress-bar-inline" role="progressbar" aria-label="Calificando la respuesta" />
      )}
      {!busy && isDisabled && disabledReason && <span className="hint gate-disabled-reason">{disabledReason}</span>}
      <span className="sr-only" aria-live="polite">
        {busy ? stage : ""}
      </span>
    </div>
  );
}
