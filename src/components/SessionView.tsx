import { SessionPlanView } from "../features/notebook/SessionPlanView";
import { RoadmapView } from "../features/roadmap/RoadmapView";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";

// The session's single view with an internal 2-state filter:
// Conversación (the live roadmap conversation) | Plan.
//
// The filter row is its OWN `.view` sibling — never a wrapper around the
// children: RoadmapView already renders `.view roadmap` and SessionPlanView
// renders `.view plan`, so nesting a `.view` here would double the wrapper.
// Keeping the children as direct children of `.workspace` is also what
// preserves RoadmapView's `closest(".workspace")` auto-scroll, the sticky
// `.composer.dock`, and the scroll sentinel staying the last node of its view.
//
// The filter values ARE the existing `"roadmap"`/`"plan"` views (see
// `stores/ui.ts`), so every existing `setView` call site (just-sealed jump,
// drawer jumps, cross-view buttons) switches this filter for free.
export function SessionView() {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const session = useRoadmap((s) => s.session);
  const hasSession = session !== null;

  // No session → no plan: fall back to the conversation instead of rendering
  // the plan with nothing to show (mirrors Header's `hasSession` gate).
  const inPlan = view === "plan" && hasSession;
  const inConversation = !inPlan;

  return (
    <>
      <nav className="view topbar-tabs session-filter" aria-label="Filtro de la sesión">
        <button
          className={"tab" + (inConversation ? " active" : "")}
          onClick={() => setView("roadmap")}
          aria-current={inConversation ? "page" : undefined}
        >
          Conversación
        </button>
        <button
          className={"tab" + (inPlan ? " active" : "")}
          onClick={() => setView("plan")}
          disabled={!hasSession}
          aria-current={inPlan ? "page" : undefined}
        >
          Plan
        </button>
      </nav>
      {inPlan ? <SessionPlanView /> : <RoadmapView />}
    </>
  );
}
