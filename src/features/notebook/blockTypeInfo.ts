/**
 * Student-facing description of each block type, shared by everything that
 * narrates generation (the draft preview, the "what is being made" card).
 * Plain Spanish, no internal identifiers, no "modelo"/"IA" jargon.
 */

/** Short name of the block, as the student would call it. */
export const BLOCK_TYPE_LABEL: Record<string, string> = {
  spaced_interleaved_retrieval: "Repaso espaciado",
  anchored_micro_theory: "Explicación",
  declarative_visual_diagram: "Diagrama",
  branching_scenario_challenge: "Escenario con decisiones",
  heuristic_error_audit: "Caza del error",
  interactive_prediction_gate: "Predicción",
  hands_on_mission: "Misión práctica",
  metacognitive_closure: "Cierre de la clase",
};

/** What this block asks of the student — shown while it is being written, so
 *  the wait says what is coming instead of only that something is. */
export const BLOCK_TYPE_PURPOSE: Record<string, string> = {
  spaced_interleaved_retrieval: "Un par de preguntas para recordar lo que ya viste, sin mirar.",
  anchored_micro_theory: "Una explicación corta con una idea concreta a la que agarrarte.",
  declarative_visual_diagram: "Un dibujo para ver cómo encajan las partes.",
  branching_scenario_challenge: "Una situación donde tú decides y ves qué pasa con cada camino.",
  heuristic_error_audit: "Algo con un error escondido: tu tarea es encontrarlo.",
  interactive_prediction_gate: "Una pregunta para que anticipes el resultado antes de verlo.",
  hands_on_mission: "Una tarea para hacer con tus manos y comprobar que lo aprendiste.",
  metacognitive_closure: "Un momento para repasar lo que lograste y qué sigue.",
};

/** The silhouette of the block, so the placeholder looks like what will land. */
export type SkeletonKind = "text" | "choices" | "diagram" | "task";

const SKELETON_BY_TYPE: Record<string, SkeletonKind> = {
  anchored_micro_theory: "text",
  declarative_visual_diagram: "diagram",
  interactive_prediction_gate: "choices",
  branching_scenario_challenge: "choices",
  spaced_interleaved_retrieval: "choices",
  heuristic_error_audit: "task",
  hands_on_mission: "task",
  metacognitive_closure: "task",
};

/** `text` when the type is not known yet (or not one we recognise): the most
 *  neutral silhouette. */
export function skeletonKindFor(blockType: string | null): SkeletonKind {
  return (blockType && SKELETON_BY_TYPE[blockType]) || "text";
}
