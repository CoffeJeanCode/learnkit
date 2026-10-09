import { useEffect } from "react";
import { Header } from "./components/Header";
import { SessionView } from "./components/SessionView";
import { UpdateBanner } from "./components/UpdateBanner";
import { ClassNotebookView } from "./features/notebook/ClassNotebookView";
import { CapabilityMapView } from "./features/capabilities/CapabilityMapView";
import { LearnerMemoryView } from "./features/learner-memory/LearnerMemoryView";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { readActiveSessionId, useRoadmap } from "./stores/roadmap";
import { useUi } from "./stores/ui";
import { useStudy, usePresentation } from "./stores/study";
import { useUpdater } from "./stores/updater";
import { logStudyEvent } from "./lib/tauri";
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
  const { showCapabilityMap } = usePresentation();

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

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const activeId = readActiveSessionId();
      if (!activeId) return;
      // `openSession` restores the conversation AND loads that session's
      // classes (see the roadmap store) — it swallows its own errors, so a
      // vanished session just leaves us on the roadmap landing.
      await openSession(activeId);
      if (!cancelled) setView("roadmap");
    })();
    return () => {
      cancelled = true;
    };
  }, [openSession, setView]);

  return (
    <div className="app">
      <Header />
      <UpdateBanner />
      <main className="workspace">
        {view === "providers" ? (
          <ProvidersView />
        ) : view === "learner-memory" ? (
          <LearnerMemoryView />
        ) : view === "capabilities" && showCapabilityMap ? (
          <CapabilityMapView />
        ) : view === "notebook" ? (
          <ClassNotebookView />
        ) : view === "roadmap" || view === "plan" ? (
          <SessionView />
        ) : (
          <RoadmapView />
        )}
      </main>
    </div>
  );
}
