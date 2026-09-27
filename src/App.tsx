import { useEffect } from "react";
import { Header } from "./components/Header";
import { ClassNotebookView } from "./features/notebook/ClassNotebookView";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { listCourseClasses, listCourses } from "./lib/tauri";
import { useNotebookNav } from "./stores/notebook";
import { useUi } from "./stores/ui";
import "./styles.css";

// Single-column layout: top bar (brand + session/settings actions) over the
// active view. No sidebar — the product is one onboarding flow.
//
// Landing rule: if the notebook store already has classes (a previous plan
// was sealed and imported), land on the classes view — not on an empty
// roadmap prompt. Otherwise the roadmap is the landing.
export default function App() {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const openNotebooks = useNotebookNav((s) => s.open);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const courses = await listCourses();
        if (cancelled || courses.length === 0) return;
        const classes = await listCourseClasses(courses[0].id);
        if (cancelled || classes.length === 0) return;
        openNotebooks(classes, classes[0].id, courses[0].id);
        setView("notebook");
      } catch {
        // Backend warming up or store empty — roadmap stays the landing.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [openNotebooks, setView]);

  return (
    <div className="app">
      <Header />
      <main className="workspace">
        {view === "providers" ? (
          <ProvidersView />
        ) : view === "notebook" ? (
          <ClassNotebookView />
        ) : (
          <RoadmapView />
        )}
      </main>
    </div>
  );
}
