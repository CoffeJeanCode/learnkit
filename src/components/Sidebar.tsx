import { useUi } from "../stores/ui";

// The app is a single onboarding flow (the Roadmap & Syllabus Diagnostic
// Agent), not a multi-tab admin panel. Providers/Chat/Agents/Workflows stay
// in the codebase for development, but only Providers (needed to configure
// an API key) is reachable from the UI, via this one settings toggle.
export function Sidebar() {
  const { view, setView } = useUi();
  const inSettings = view === "providers";

  return (
    <aside className="sidebar">
      <div className="brand">LearnKit</div>
      <div className="brand-sub">tu mentor de aprendizaje</div>
      <div className="sidebar-foot">
        <button className="nav-item" onClick={() => setView(inSettings ? "roadmap" : "providers")}>
          {inSettings ? "← Volver al onboarding" : "⚙ Proveedores (API keys)"}
        </button>
      </div>
    </aside>
  );
}
