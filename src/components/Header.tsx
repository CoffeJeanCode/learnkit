import { useState } from "react";
import { useNotebookNav } from "../stores/notebook";
import { displaySessionTitle, useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { SessionsDrawer } from "./SessionsDrawer";

// Top bar replacing the old sidebar: brand + the ACTIVE SESSION's three
// sections (Conversación | Plan | Clases) on the left, session + settings
// actions on the right. The session is the container: its chip names it, and
// the tabs only exist for sections that session actually has — "Clases" only
// once it owns a course (i.e. it's sealed), so a deleted session can't leave
// a tab pointing at classes that are already gone.
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
  const inConversation = view === "roadmap";
  const inPlan = view === "plan";
  const inNotebook = view === "notebook";
  const hasSession = session !== null;
  const courseId = session?.imported_course_id ?? null;
  // The course is created the moment the session seals, so it's a reliable
  // "this session has classes" signal even before they're loaded.
  const hasClasses = courseId !== null;
  const chipTitle = displaySessionTitle(session);

  const openConversation = () => setView("roadmap");
  const openPlan = () => setView("plan");
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
      // notebook; the Plan/Conversación tabs stay usable.
    } finally {
      setOpeningClasses(false);
    }
  };

  return (
    <>
      <header className="topbar">
        <div className="topbar-left">
          {!inSettings && (
            <button className="drawer-toggle" onClick={() => setDrawerOpen(true)} title="Ver sesiones guardadas">
              ☰
            </button>
          )}
          <div className="brand">LearnKit</div>
          {hasSession && chipTitle && !inSettings && (
            <span className="session-chip" title={chipTitle}>
              {chipTitle}
            </span>
          )}
          {!inSettings && (
            <nav className="topbar-tabs" aria-label="Secciones de la sesión">
              <button
                className={"tab" + (inConversation ? " active" : "")}
                onClick={openConversation}
                aria-current={inConversation ? "page" : undefined}
              >
                Conversación
              </button>
              {hasSession && (
                <button
                  className={"tab" + (inPlan ? " active" : "")}
                  onClick={openPlan}
                  aria-current={inPlan ? "page" : undefined}
                >
                  Plan
                </button>
              )}
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
          {!inSettings && (
            <button onClick={() => { reset(); setView("roadmap"); }} title="Empieza una conversación nueva">
              Nueva sesión
            </button>
          )}
          <button onClick={() => setView(inSettings ? lastMainView : "providers")}>
            {inSettings ? "← Volver" : "⚙ Proveedores"}
          </button>
        </div>
      </header>
      <SessionsDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </>
  );
}
