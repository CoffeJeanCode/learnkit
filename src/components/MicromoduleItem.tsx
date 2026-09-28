import type { Micromodule } from "../lib/schemas";

/** One study session inside a week of the plan — the SAME markup for the
 *  proposal card, the saved plan and the notebook's Plan tab, so it can't
 *  drift between them. Shows only what the student needs to act: the
 *  session's title/hours, its conceptual focus, the artifact it produces,
 *  and the capability gained. `interactiveBlocks` is internal orchestrator
 *  metadata (see `domain::roadmap::Micromodule`) and is NEVER rendered here
 *  — the student must never see raw catalog names like "socratic
 *  prediction" or "metacognitive closure". */
export function MicromoduleItem({ mod, className }: { mod: Micromodule; className?: string }) {
  return (
    <li className={className}>
      <strong>
        {mod.label} ({mod.hours} h)
      </strong>
      {mod.focus && (
        <p className="mod-focus">
          <strong>Foco:</strong> {mod.focus}
        </p>
      )}
      <p className="mod-deliverable">
        <strong>Entregable:</strong> {mod.deliverable}
      </p>
      {mod.objective && (
        <p className="mod-objective">
          <strong>Objetivo:</strong> {mod.objective}
        </p>
      )}
    </li>
  );
}
