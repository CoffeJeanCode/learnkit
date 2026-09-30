import type { DeliverableArtifactType, Micromodule } from "../lib/schemas";

// Student-facing label for each closed `artifactType` bucket — shown as a
// small badge next to the deliverable's description so the student sees
// AT A GLANCE what kind of artifact this session produces, not just its
// free-text description (see `Deliverable` in `src-tauri/src/domain/roadmap.rs`).
const ARTIFACT_TYPE_LABEL: Record<DeliverableArtifactType, string> = {
  tests_passing: "Tests en verde",
  formal_diagram: "Diagrama formal",
  functional_cli: "CLI funcional",
  diagnostic_matrix: "Matriz de diagnóstico",
  working_demo: "Demo funcionando",
  other: "Otro",
};

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
        <strong>Entregable:</strong> <span className="badge">{ARTIFACT_TYPE_LABEL[mod.deliverable.artifactType]}</span>{" "}
        {mod.deliverable.description}
      </p>
      {mod.objective && (
        <p className="mod-objective">
          <strong>Objetivo:</strong> {mod.objective}
        </p>
      )}
    </li>
  );
}
