import { useState } from "react";
import { useNotebookNav } from "../stores/notebook";
import { displaySessionTitle, useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { SessionsDrawer } from "./SessionsDrawer";

// Top bar replacing the old sidebar: brand + the ACTIVE SESSION's two
// sections (Sesión | Clases) on the left, session + settings actions on the
// right. "Sesión" is the merged Conversación/Plan section — its internal
// Conversación|Plan filter lives inside `SessionView`, not here. The session
// is the container: its chip names it, and the tabs only exist for sections
// that session actually has — "Clases" only once it owns a course (i.e. it's
// sealed), so a deleted session can't leave a tab pointing at classes that
// are already gone.
//
// "Nueva sesión" only RESETS the draft state and goes home — it never
// persists anything (a session is created in the DB when the first message
// actually sends). The old second button ("Nueva clase") did exactly the
// same thing under a different label, so it's gone.
export function Header() {
  const { view, lastMainView, setView } = useUi();
  const reset = useRoadmap((s) => s.reset);
  const session = useRoadmap((s) => s.session);
  const openCourse = useNotebookNav((s) => s.openCourse);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [openingClasses, setOpeningClasses] = useState(false);

  const inSettings = view === "providers";
  const inMemory = view === "learner-memory";
  // Both settings and the learner-memory viewer are full-screen overlays
  // over the active session — neither shows the session chip/tabs.
  const inOverlay = inSettings || inMemory;
  // One merged tab for both session sections: the Conversación|Plan filter
  // now lives INSIDE `SessionView`, so the header only tracks whether we're
  // in the session view at all.
  const inSession = view === "roadmap" || view === "plan";
  const inNotebook = view === "notebook";
  const hasSession = session !== null;
  const courseId = session?.imported_course_id ?? null;
  // The course is created the moment the session seals, so it's a reliable
  // "this session has classes" signal even before they're loaded.
  const hasClasses = courseId !== null;
  const chipTitle = displaySessionTitle(session);

  const openSession = () => {
    // Already inside the session view → keep whichever filter is showing;
    // from anywhere else (Clases, ajustes) resume the last session section.
    if (inSession) return;
    setView(lastMainView === "plan" ? "plan" : "roadmap");
  };
  const openClasses = async () => {
    if (!courseId || openingClasses) return;
    setOpeningClasses(true);
    try {
      // Loads (or reloads) this session's classes and resumes the last one
      // read — the store already handles the stale-class fallback.
      await openCourse(courseId);
      setView("notebook");
    } catch {
      // Leave the student where they are rather than landing on an empty
      // notebook; the Sesión tab stays usable.
    } finally {
      setOpeningClasses(false);
    }
  };

  return (
    <>
      <header className="topbar">
        <div className="topbar-left">
          {!inOverlay && (
            <button className="drawer-toggle" onClick={() => setDrawerOpen(true)} title="Ver sesiones guardadas">
              ☰
            </button>
          )}
          <div className="brand">LearnKit</div>
          {hasSession && chipTitle && !inOverlay && (
            <span className="session-chip" title={chipTitle}>
              {chipTitle}
            </span>
          )}
          {!inOverlay && (
            <nav className="topbar-tabs" aria-label="Secciones de la sesión">
              <button
                className={"tab" + (inSession ? " active" : "")}
                onClick={openSession}
                aria-current={inSession ? "page" : undefined}
              >
                Sesión
              </button>
              {hasClasses && (
                <button
                  className={"tab" + (inNotebook ? " active" : "")}
                  onClick={openClasses}
                  disabled={openingClasses}
                  aria-current={inNotebook ? "page" : undefined}
                >
                  Clases
                </button>
              )}
            </nav>
          )}
        </div>
        <div className="topbar-actions">
          {!inOverlay && (
            <button onClick={() => { reset(); setView("roadmap"); }} title="Empieza una conversación nueva">
              Nueva sesión
            </button>
          )}
          <button onClick={() => setView(inMemory ? lastMainView : "learner-memory")}>
            {inMemory ? "← Volver" : "🧠 Memoria"}
          </button>
          <button onClick={() => setView(inSettings ? lastMainView : "providers")}>
            {inSettings ? "← Volver" : "⚙ Proveedores"}
          </button>
        </div>
      </header>
      <SessionsDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </>
  );
}
