import { useEffect, useRef } from "react";
import { Header } from "./components/Header";
import { SessionView } from "./components/SessionView";
import { UpdateBanner } from "./components/UpdateBanner";
import { ClassNotebookView } from "./features/notebook/ClassNotebookView";
import { GraphView } from "./features/graph/GraphView";
import { DevPanel } from "./features/dev/DevPanel";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { useNotebookNav } from "./stores/notebook";
import { readActiveSessionId, useRoadmap } from "./stores/roadmap";
import { useUi } from "./stores/ui";
import { useDev } from "./stores/dev";
import { useStudy } from "./stores/study";
import { useUpdater } from "./stores/updater";
import { logStudyEvent } from "./lib/tauri";
import { browserStore, saveResume } from "./lib/resume";
import { isRestoring, resumeSession } from "./lib/resumeSession";
import "./styles.css";

// Single-column layout: top bar (brand + the active session's section tabs)
// over the active view. No sidebar — the product is one onboarding flow.
//
// Landing rule: reopen the session the student was last inside (persisted as
// `learnkit-active-session`) and land on its Conversación. The guess this
// replaces — "whichever course is newest in SQLite" — is what left a dead
// Clases tab standing after its session was deleted. With no persisted
// session (fresh install, or it was deleted) the roadmap prompt is the
// landing, and no session-scoped tab exists at all.
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

  // `.workspace` is the page's only scroller and survives class changes, so
  // opening a class (from the roadmap or the class path) would otherwise land
  // wherever the previous screen was scrolled. Scoped to the notebook: the
  // roadmap chat manages its own scroll-to-latest.
  useEffect(() => {
    if (view === "notebook") workspaceRef.current?.scrollTo({ top: 0 });
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
      const activeId = readActiveSessionId();
      if (!activeId) return;
      // Reopen the session and land where it was left (the class and block inside
      // the notebook, or the plan / conversation). It swallows its own errors, so a
      // vanished session just leaves us on the roadmap landing.
      await resumeSession(activeId);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Remember where the student is, per session, so reopening it lands here. Quiet while a
  // session is being restored: the screen the app starts on must not overwrite the saved one.
  const sessionId = useRoadmap((s) => s.session?.session_id);
  useEffect(() => {
    if (!sessionId || isRestoring()) return;
    if (view !== "roadmap" && view !== "plan" && view !== "notebook") return;
    saveResume(browserStore(), sessionId, view === "notebook" ? { view, classId: activeClassId ?? undefined } : { view });
  }, [sessionId, view, activeClassId]);

  return (
    <div className="app">
      <Header />
      <UpdateBanner />
      <main className="workspace" ref={workspaceRef}>
        {view === "providers" ? (
          <ProvidersView />
        ) : view === "graph" ? (
          <GraphView />
        ) : view === "notebook" ? (
          <ClassNotebookView />
        ) : view === "roadmap" || view === "plan" ? (
          <SessionView />
        ) : (
          <RoadmapView />
        )}
      </main>
      {devEnabled && <DevPanel />}
    </div>
  );
}
