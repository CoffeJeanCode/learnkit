import type { Micromodule } from "../lib/schemas";

/** One module inside a week of the plan — the SAME markup for the proposal
 *  card, the saved plan and the notebook's Plan tab, so the learning
 *  objective can't drift between them. The objective is the line that
 *  answers "¿qué me llevo de este módulo?" (what you'll be able to DO);
 *  the deliverable stays what you hand in. Rendered only when present —
 *  syllabi sealed before `Micromodule::objective` existed have none. */
export function MicromoduleItem({ mod, className }: { mod: Micromodule; className?: string }) {
  return (
    <li className={className}>
      <strong>
        {mod.label} ({mod.hours} h)
      </strong>{" "}
      — {mod.deliverable}.{" "}
      {mod.interactiveBlocks.map((b) => b.replace(/_/g, " ")).join(" · ")}
      {mod.objective && <span className="mod-objective">Objetivo: {mod.objective}</span>}
    </li>
  );
}
