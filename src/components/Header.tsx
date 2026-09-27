import { useState } from "react";
import { useNotebookNav } from "../stores/notebook";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { SessionsDrawer } from "./SessionsDrawer";

// Top bar replacing the old sidebar: brand + drawer toggle on the left,
// session + settings actions on the right. "Nueva sesión"/"Nueva clase" only
// RESETS the draft state and goes home — it never persists anything (a
// session is created in the DB when the first message actually sends).
export function Header() {
  const { view, setView } = useUi();
  const reset = useRoadmap((s) => s.reset);
  const hasClasses = useNotebookNav((s) => s.classes.length > 0);
  const inSettings = view === "providers";
  const inNotebook = view === "notebook";
  const [drawerOpen, setDrawerOpen] = useState(false);

  const goHomeForNewClass = () => {
    reset();
    setView("roadmap");
  };

  return (
    <>
      <header className="topbar">
        <div className="topbar-left">
          {!inSettings && (
            <button
              className="drawer-toggle"
              onClick={() => setDrawerOpen(true)}
              title="Ver sesiones guardadas"
            >
              ☰
            </button>
          )}
          <div className="brand">LearnKit</div>
        </div>
        <div className="topbar-actions">
          {!inSettings && (
            <button onClick={goHomeForNewClass} title="Vuelve al inicio para conversar sobre algo nuevo">
              {inNotebook ? "Nueva clase" : "Nueva sesión"}
            </button>
          )}
          {!inSettings && !inNotebook && hasClasses && (
            <button onClick={() => setView("notebook")} title="Volver a tus clases">
              Mis clases
            </button>
          )}
          <button onClick={() => setView(inSettings ? "roadmap" : "providers")}>
            {inSettings ? "← Volver" : "⚙ Proveedores"}
          </button>
        </div>
      </header>
      <SessionsDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </>
  );
}
