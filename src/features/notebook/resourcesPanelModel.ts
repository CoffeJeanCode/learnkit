import type { ClassResource, CourseResearch } from "../../lib/schemas";

/** What the "Recursos" panel shows for a class. */
export type PanelView =
  /** There are curated resources: show them (even while a later pass still runs). */
  | { kind: "list"; resources: ClassResource[] }
  /** The research has not finished (or has not even started): nothing to show yet. */
  | { kind: "searching" }
  /** No search key is configured, so nothing was attempted. */
  | { kind: "needs_key" }
  | { kind: "failed"; detail: string | null }
  /** The research finished and found nothing usable for this class. */
  | { kind: "none" };

/**
 * Decides the panel from the course's research state and the class's resources.
 * `research` is `null` until the first read returns.
 *
 * Resources win over everything: a class that has them shows them even when
 * the course's research is running again or failed afterwards.
 */
export function panelView(research: CourseResearch | null, resources: ClassResource[]): PanelView {
  if (resources.length > 0) return { kind: "list", resources };
  if (research === null) return { kind: "searching" };
  switch (research.status) {
    case null:
    case "pending":
    case "running":
      return { kind: "searching" };
    case "skipped_no_key":
      return { kind: "needs_key" };
    case "failed":
      return { kind: "failed", detail: research.detail };
    case "done":
      return { kind: "none" };
  }
}

/**
 * Whether opening this panel should start the course's research. Never for a
 * run in progress, a finished one, or a failure (a failure waits for the
 * student's "Reintentar" — it may cost model calls). A course that predates the
 * feature (`status: null`) starts once; a skip because of a missing key starts
 * again the moment a key exists.
 */
export function shouldAutoStart(research: CourseResearch | null, keyConfigured: boolean): boolean {
  if (research === null) return false;
  if (research.status === null) return true;
  return research.status === "skipped_no_key" && keyConfigured;
}

export const KIND_LABEL: Record<ClassResource["kind"], string> = {
  tutorial: "Tutorial",
  documentation: "Documentación",
  video: "Video",
  exercise: "Ejercicio",
  reference: "Referencia",
};
