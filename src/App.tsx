import { useEffect, useRef, useState } from "react";
import { Header } from "./components/Header";
import { SessionView } from "./components/SessionView";
import { UpdateBanner } from "./components/UpdateBanner";
import { ClassNotebookView } from "./features/notebook/ClassNotebookView";
import { GraphView } from "./features/graph/GraphView";
import { DevPanel } from "./features/dev/DevPanel";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { GalleryStudioView } from "./features/gallery/GalleryStudioView";
import { useNotebookNav } from "./stores/notebook";
import { readActiveSessionId, useRoadmap } from "./stores/roadmap";
import { useUi } from "./stores/ui";
import { useDev } from "./stores/dev";
import { useStudy } from "./stores/study";
import { useUpdater } from "./stores/updater";
import { listRoadmapSessions, logStudyEvent } from "./lib/tauri";
import { browserStore, saveResume } from "./lib/resume";
import { isRestoring, resumeSession, useResumeTarget } from "./lib/resumeSession";
import "./styles.css";

// Single-column layout: top bar (brand + the active session's section tabs)
// over the active view. No sidebar — the product is one onboarding flow.
//
// Landing rule: reopen the session the student was last inside (persisted as
// `learnkit-active-session`) and land on its last learning position (class and
// block inside the notebook, or conversation/plan). Falling back to the latest
// sealed session if the key is empty, or the roadmap landing if fresh.
export default function App() {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const openSession = useRoadmap((s) => s.openSession);
  const checkOnStartup = useUpdater((s) => s.checkOnStartup);
  const loadStudy = useStudy((s) => s.load);
  const initDev = useDev((s) => s.init);
  const devEnabled = useDev((s) => s.enabled);
  const activeClassId = useNotebookNav((s) => s.activeClassId);
  const workspaceRef = useRef<HTMLElement>(null);
  const prevActiveClassRef = useRef<string | null>(null);
  const [bootstrapping, setBootstrapping] = useState(true);

  // Keep track of activeClassId to avoid unnecessary scroll resets.
  // ClassNotebookView handles smooth auto-scrolling to the active block
  // on mount, on view return, and as new blocks unlock.
  useEffect(() => {
    if (view === "notebook" && activeClassId) {
      prevActiveClassRef.current = activeClassId;
    }
  }, [view, activeClassId]);

  // Check for a new version once on entry. Silent by design: no release
  // published yet, or starting offline, must not surface anything.
  useEffect(() => {
    void checkOnStartup();
  }, [checkOnStartup]);

  // Study switch: load the version in force and log the app opening (best-effort).
  useEffect(() => {
    void loadStudy();
    void logStudyEvent("app_opened").catch(() => undefined);
  }, [loadStudy]);

  // Developer mode: restore the switch and subscribe to generation traces.
  useEffect(() => {
    void initDev();
  }, [initDev]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        let activeId = readActiveSessionId();
        if (!activeId) {
          try {
            const sessions = await listRoadmapSessions();
            if (cancelled) return;
            const best = sessions.find((s) => s.status === "sealed") ?? sessions[0];
            if (best) activeId = best.session_id;
          } catch {
            // No sessions available
          }
        }
        if (!activeId || cancelled) return;
        // Reopen the session and land where it was left (the class and block inside
        // the notebook, or the plan / conversation). It swallows its own errors, so a
        // vanished session just leaves us on the roadmap landing.
        await resumeSession(activeId);
      } finally {
        if (!cancelled) {
          setBootstrapping(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Remember where the student is, per session, so reopening it lands here. Quiet while a
  // session is being restored: the screen the app starts on must not overwrite the saved one.
  const sessionId = useRoadmap((s) => s.session?.session_id);
  useEffect(() => {
    if (!sessionId || isRestoring() || bootstrapping) return;
    if (view !== "roadmap" && view !== "plan" && view !== "notebook") return;
    saveResume(browserStore(), sessionId, view === "notebook" ? { view, classId: activeClassId ?? undefined } : { view });
  }, [sessionId, view, activeClassId, bootstrapping]);

  return (
    <div className="app">
      <Header />
      <UpdateBanner />
      <main className={`workspace${view === "graph" ? " is-graph" : ""}`} ref={workspaceRef}>
        {bootstrapping ? (
          <div
            className="view-loading"
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              minHeight: "50vh",
              gap: "1rem",
              color: "var(--color-muted)",
            }}
          >
            <div className="spinner" />
            <p>Continuando tu sesión de aprendizaje…</p>
          </div>
        ) : (
          <>
            <div
              className="view-wrapper"
              style={{ display: view === "notebook" ? "contents" : "none" }}
            >
              <ClassNotebookView />
            </div>
            <div
              className="view-wrapper"
              style={{
                display:
                  view === "roadmap" || view === "plan" ? "contents" : "none",
              }}
            >
              <SessionView />
            </div>
            {view === "graph" && <GraphView />}
            {view === "gallery" && <GalleryStudioView />}
            {view === "providers" && <ProvidersView />}
          </>
        )}
      </main>
      {devEnabled && <DevPanel />}
    </div>
  );
}
