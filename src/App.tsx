import { Sidebar } from "./components/Sidebar";
import { ProvidersView } from "./features/providers/ProvidersView";
import { RoadmapView } from "./features/roadmap/RoadmapView";
import { useUi } from "./stores/ui";
import "./styles.css";

// Chat/Agents/Workflows views (the original multi-agent playground) stay in
// the codebase for development but are intentionally not routed here: the
// product is a single onboarding flow, not a multi-tab admin panel. Restore
// their branches here if that changes.
export default function App() {
  const view = useUi((s) => s.view);
  return (
    <div className="app">
      <Sidebar />
      <main className="workspace">
        {view === "providers" && <ProvidersView />}
        {view !== "providers" && <RoadmapView />}
      </main>
    </div>
  );
}
