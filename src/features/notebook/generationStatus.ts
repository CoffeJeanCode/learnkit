import type { GenerationStage } from "../../components/AgentLoader";
import type { BlockDraft } from "./blockDraft";
import { BLOCK_TYPE_LABEL, BLOCK_TYPE_PURPOSE, type SkeletonKind, skeletonKindFor } from "./blockTypeInfo";

/** What the student is told is happening while the next block is made. */
export interface GenerationStatus {
  /** One line, what is going on right now. */
  headline: string;
  /** A second line with the why / what is coming. */
  detail: string;
  skeleton: SkeletonKind;
  /** Drives the accent of the card; also lets tests assert the branch taken. */
  phase: "preparing" | "thinking" | "writing" | "retrying";
}

/**
 * The honest status line for a block in progress, from real signals only:
 * the generator's own stage (`agent://started` count), and the live draft
 * (`notebook_block://delta`: block type, text so far, reasoning amount).
 * Never a percentage or an ETA — nothing in the pipeline knows one.
 *
 * Priority, most specific first:
 *  1. a retry (second attempt, by stage or by draft) — the student should
 *     know the first try was discarded rather than see progress go backwards;
 *  2. the block type is known — say what is being made and what it asks;
 *  3. the model is reasoning (an amount arrived, no type yet);
 *  4. the generator started, nothing arrived yet;
 *  5. nothing started yet.
 */
export function describeGeneration(stage: GenerationStage, draft: BlockDraft | null): GenerationStatus {
  const retrying = stage === "reintentando" || (draft?.attempt ?? 1) > 1;
  const blockType = draft?.blockType ?? null;
  const skeleton = skeletonKindFor(blockType);

  if (retrying) {
    return {
      phase: "retrying",
      skeleton,
      headline: "Ajustando este paso…",
      detail: "El primer intento no quedó como quería, así que lo estoy rehaciendo.",
    };
  }
  if (blockType && BLOCK_TYPE_LABEL[blockType]) {
    const hasText = draft !== null && Object.keys(draft.fields).length > 0;
    return {
      phase: "writing",
      skeleton,
      headline: `${hasText ? "Escribiendo" : "Armando"}: ${BLOCK_TYPE_LABEL[blockType]}`,
      detail: BLOCK_TYPE_PURPOSE[blockType] ?? "",
    };
  }
  if (draft && draft.reasoningChars > 0) {
    return {
      phase: "thinking",
      skeleton,
      headline: "Pensando cómo plantearlo…",
      detail: "Decidiendo qué te conviene practicar ahora, según lo que ya hiciste.",
    };
  }
  if (stage === "escribiendo") {
    return {
      phase: "writing",
      skeleton,
      headline: "Escribiendo el siguiente paso…",
      detail: "Lo estoy redactando a tu medida.",
    };
  }
  return {
    phase: "preparing",
    skeleton,
    headline: "Preparando el siguiente paso…",
    detail: "Mirando lo que ya hiciste para decidir qué sigue.",
  };
}
