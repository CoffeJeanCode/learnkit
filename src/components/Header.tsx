import { useState } from "react";
import { useNotebookNav } from "../stores/notebook";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { SessionsDrawer } from "./SessionsDrawer";

// Top bar replacing the old sidebar: brand + section tabs (Conversación |
// Clases) on the left, session + settings actions on the right. The tabs
// are the primary two-way navigation between the mentor conversation and
// the class notebook — always visible, active state highlighted. The
// sessions journal (☰) belongs to the conversation only, per the flow
// decision: switching courses happens from there, never from inside a
// class.
//
// "Nueva sesión"/"Nueva clase" only RESETS the draft state and goes home —
// it never persists anything (a session is created in the DB when the
// first message actually sends).
export function Header() {
  const { view, lastMainView, setView } = useUi();
  const reset = useRoadmap((s) => s.reset);
  const hasClasses = useNotebookNav((s) => s.classes.length > 0);
  const inSettings = view === "providers";
  const inNotebook = view === "notebook";
  const inConversation = view === "roadmap";
  const [drawerOpen, setDrawerOpen] = useState(false);

  const goHomeForNewClass = () => {
    reset();
    setView("roadmap");
  };

  return (
    <>
      <header className="topbar">
        <div className="topbar-left">
          {inConversation && (
            <button
              className="drawer-toggle"
              onClick={() => setDrawerOpen(true)}
              title="Ver sesiones guardadas"
            >
              ☰
            </button>
          )}
          <div className="brand">LearnKit</div>
          {!inSettings && (
            <nav className="topbar-tabs" aria-label="Secciones">
              <button
                className={"tab" + (inConversation ? " active" : "")}
                onClick={() => setView("roadmap")}
                aria-current={inConversation ? "page" : undefined}
              >
                Conversación
              </button>
              {hasClasses && (
                <button
                  className={"tab" + (inNotebook ? " active" : "")}
                  onClick={() => setView("notebook")}
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
            <button onClick={goHomeForNewClass} title="Vuelve al inicio para conversar sobre algo nuevo">
              {inNotebook ? "Nueva clase" : "Nueva sesión"}
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
