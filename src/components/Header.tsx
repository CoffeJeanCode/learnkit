import { useState } from "react";
import { queueBlockResume } from "../lib/resumeSession";
import { useDev } from "../stores/dev";
import { useNotebookNav } from "../stores/notebook";
import { displaySessionTitle, useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { useUpdater } from "../stores/updater";
import { SessionsDrawer } from "./SessionsDrawer";
import {
  IconArrowLeft,
  IconGallery,
  IconGear,
  IconGraph,
  IconMenu,
  IconPencil,
  IconRefresh,
  IconWrench,
} from "./Icons";

export function Header() {
  const { view, lastMainView, setView } = useUi();
  const reset = useRoadmap((s) => s.reset);
  const session = useRoadmap((s) => s.session);
  const openCourse = useNotebookNav((s) => s.openCourse);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [openingClasses, setOpeningClasses] = useState(false);

  const devEnabled = useDev((s) => s.enabled);
  const devOpen = useDev((s) => s.panelOpen);
  const setDevOpen = useDev((s) => s.setPanelOpen);
  const inSettings = view === "providers";
  const inGraph = view === "graph";
  const inGallery = view === "gallery";
  // Settings, graph, and gallery are full-screen overlays
  const inOverlay = inSettings || inGraph || inGallery;
  const inSession = view === "roadmap" || view === "plan";
  const inNotebook = view === "notebook";
  const hasSession = session !== null;
  const courseId = session?.imported_course_id ?? null;
  const hasClasses = courseId !== null;
  const chipTitle = displaySessionTitle(session);

  // Update checking
  const updaterStatus = useUpdater((s) => s.status);
  const update = useUpdater((s) => s.update);
  const updaterError = useUpdater((s) => s.error);
  const checkNow = useUpdater((s) => s.checkNow);
  const install = useUpdater((s) => s.install);
  const updaterBusy = updaterStatus === "checking" || updaterStatus === "installing";
  const hasUpdate = updaterStatus === "available" && !!update;
  const updateTitle =
    updaterStatus === "error" && updaterError
      ? `No se pudo comprobar la versión: ${updaterError}`
      : hasUpdate
        ? (update.body ?? `Hay una versión nueva: ${update.version}`)
        : "Comprobar actualizaciones de LearnKit";

  const openSession = () => {
    if (inSession) return;
    setView(lastMainView === "plan" ? "plan" : "roadmap");
  };

  const openClasses = async () => {
    if (!courseId || openingClasses) return;
    setOpeningClasses(true);
    try {
      await openCourse(courseId);
      if (session) queueBlockResume(session.session_id);
      setView("notebook");
    } finally {
      setOpeningClasses(false);
    }
  };

  return (
    <>
      <header className="topbar">
        <div className="topbar-left">
          {!inOverlay && (
            <button
              className="drawer-toggle"
              onClick={() => setDrawerOpen(true)}
              title="Ver sesiones guardadas"
              aria-label="Abrir menú de sesiones"
            >
              <IconMenu />
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
            <button
              className="topbar-action-btn"
              onClick={() => { reset(); setView("roadmap"); }}
              title="Empieza una conversación nueva"
            >
              <IconPencil />
              <span>Nueva sesión</span>
            </button>
          )}

          {/* Dev Mode Actions: only rendered when dev mode is explicitly enabled */}
          {devEnabled && (
            <>
              <button
                className={`topbar-action-btn ${devOpen ? "active" : ""}`}
                onClick={() => setDevOpen(!devOpen)}
                title="Inspector de generación y telemetría"
              >
                <IconWrench />
                <span>Dev</span>
              </button>
              <button
                className={`topbar-action-btn ${inGallery ? "active" : ""}`}
                onClick={() => setView(inGallery ? lastMainView : "gallery")}
                title="Galería de todos los bloques pedagógicos"
              >
                {inGallery ? <IconArrowLeft /> : <IconGallery />}
                <span>{inGallery ? "Volver" : "Galería"}</span>
              </button>
            </>
          )}

          {/* Core student tools */}
          <button
            className={`topbar-action-btn ${inGraph ? "active" : ""}`}
            onClick={() => setView(inGraph ? lastMainView : "graph")}
            title="Tu mapa de ideas: conecta lo que aprendes con tus propias palabras"
          >
            {inGraph ? <IconArrowLeft /> : <IconGraph />}
            <span>{inGraph ? "Volver" : "🕸 Grafo"}</span>
          </button>

          <button
            className={`topbar-action-btn ${inSettings ? "active" : ""}`}
            onClick={() => setView(inSettings ? lastMainView : "providers")}
            title="Configuración de modelos y claves"
          >
            {inSettings ? <IconArrowLeft /> : <IconGear />}
            <span>{inSettings ? "Volver" : "Configuración"}</span>
          </button>

          {/* Subtle updater indicator */}
          {hasUpdate ? (
            <button
              className="topbar-action-btn update-ready"
              onClick={() => void install()}
              disabled={updaterBusy}
              title={updateTitle}
            >
              <IconRefresh />
              <span>Actualizar v{update?.version}</span>
            </button>
          ) : (
            <button
              className="topbar-action-icon"
              onClick={() => void checkNow()}
              disabled={updaterBusy}
              title={updateTitle}
              aria-label="Comprobar actualizaciones"
            >
              <IconRefresh className={updaterBusy ? "spin" : ""} />
            </button>
          )}
        </div>
      </header>
      <SessionsDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </>
  );
}
