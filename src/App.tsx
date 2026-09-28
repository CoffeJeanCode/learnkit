import { useEffect } from "react";
import { Header } from "./components/Header";
import { ClassNotebookView } from "./features/notebook/ClassNotebookView";
import { SessionPlanView } from "./features/notebook/SessionPlanView";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { readActiveSessionId, useRoadmap } from "./stores/roadmap";
import { useUi } from "./stores/ui";
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
      <main className="workspace">
        {view === "providers" ? (
          <ProvidersView />
        ) : view === "notebook" ? (
          <ClassNotebookView />
        ) : view === "plan" ? (
          <SessionPlanView />
        ) : (
          <RoadmapView />
        )}
      </main>
    </div>
  );
}
