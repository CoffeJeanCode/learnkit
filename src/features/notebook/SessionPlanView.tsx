import { useEffect, useState } from "react";
import { tauriError } from "../../lib/tauri";
import { useNotebookNav } from "../../stores/notebook";
import { useRoadmap } from "../../stores/roadmap";
import { useUi } from "../../stores/ui";
import { SessionPlanTab } from "./SessionPlanTab";

/** The header's "Plan" section — the sealed roadmap that produced the active
 *  session's course, plus the class path's locks/checks. A first-class view
 *  instead of a tab inside the notebook, so "← Ver conversación" can never
 *  bounce the student back into the class (the notebook used to auto-jump to
 *  itself the moment it mounted with a sealed session).
 *
 *  Owns no data of its own: the session comes from the roadmap store (the
 *  container the header is showing), the classes from the notebook nav. */
export function SessionPlanView() {
  const session = useRoadmap((s) => s.session);
  const opening = useRoadmap((s) => s.opening);
  const { classes, courseId, activeClassId, openCourse, clear } = useNotebookNav();
  const setActiveClass = useNotebookNav((s) => s.setActiveClass);
  const setView = useUi((s) => s.setView);
  const [loadError, setLoadError] = useState<{ course: string; message: string } | null>(null);

  const targetCourseId = session?.imported_course_id ?? null;
  // Still loading while the nav store holds another session's course (or
  // none at all) and this session actually has one to load.
  const loading = !!targetCourseId && courseId !== targetCourseId && loadError?.course !== targetCourseId;
  const classesReady = !targetCourseId || courseId === targetCourseId;

  useEffect(() => {
    if (!targetCourseId) {
      // A session with no imported course (still in the diagnostic, or a
      // pre-import legacy seal) must not keep the PREVIOUS session's classes
      // on screen — they belong to a different container.
      if (courseId) clear();
      return;
    }
    // Keyed by course so a failure can't retry forever: the failed load
    // clears the nav store, which would otherwise re-trigger this effect.
    if (loadError?.course === targetCourseId || courseId === targetCourseId) return;
    let cancelled = false;
    openCourse(targetCourseId).catch((e) => {
      if (cancelled) return;
      setLoadError({ course: targetCourseId, message: tauriError(e).message });
      clear();
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [targetCourseId, courseId, loadError]);

  if (!session && !opening) {
    return (
      <div className="view plan">
        <div className="plan-tab">
          <div className="plan-tab-head">
            <div>
              <h2>Tu plan</h2>
              <p className="muted">Todavía no hay una sesión abierta. Empieza una desde Conversación.</p>
            </div>
            <div className="row">
              <button className="btn-primary" onClick={() => setView("roadmap")}>
                Ir a Conversación
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="view plan">
      <SessionPlanTab
        session={session}
        loading={loading || opening}
        classes={classesReady ? classes : []}
        activeClassId={activeClassId}
        onSelectClass={(id) => {
          setActiveClass(id);
          setView("notebook");
        }}
        onOpenConversation={() => setView("roadmap")}
        error={loadError?.course === targetCourseId ? loadError.message : null}
      />
    </div>
  );
}
