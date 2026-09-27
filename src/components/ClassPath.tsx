import type { ClassRecord } from "../lib/schemas";

/** Sequential list of a course's classes rendered as a Duolingo-style path
 *  of connected dots — shared between the plan view (browse everything
 *  right after sealing, or after reopening a saved session) and the class
 *  notebook (jump between siblings) so both stay visually identical.
 *
 *  The nodes mirror the backend's sequential-unlock rule
 *  (`start_class_notebook`): a class locks — padlock, disabled — while any
 *  earlier class isn't `complete` yet, and only finished classes check off. */
export function ClassPath({
  classes,
  activeClassId,
  onSelect,
}: {
  classes: ClassRecord[];
  activeClassId?: string | null;
  onSelect: (classId: string) => void;
}) {
  const activeIndex = activeClassId ? classes.findIndex((c) => c.id === activeClassId) : -1;

  return (
    <div className="class-path">
      {classes.map((c, i) => {
        const locked = i !== activeIndex && classes.slice(0, i).some((prev) => !prev.complete);
        return (
          <button
            key={c.id}
            className={
              "class-path-node" +
              (c.id === activeClassId ? " active" : "") +
              (c.complete ? " done" : "") +
              (locked ? " locked" : "")
            }
            disabled={locked}
            title={locked ? "Completa las clases anteriores para desbloquearla" : undefined}
            onClick={() => onSelect(c.id)}
          >
            <span className="class-path-dot">
              {locked ? (
                <svg className="lock-icon" viewBox="0 0 14 14" width="13" height="13" aria-hidden="true">
                  <path d="M4.4 6.4 V4.7 a2.6 2.6 0 0 1 5.2 0 V6.4" />
                  <rect x="2.7" y="6.4" width="8.6" height="5.4" rx="1.4" />
                </svg>
              ) : c.complete ? (
                "✓"
              ) : (
                i + 1
              )}
            </span>
            <span className="class-path-label">{c.title}</span>
          </button>
        );
      })}
    </div>
  );
}
