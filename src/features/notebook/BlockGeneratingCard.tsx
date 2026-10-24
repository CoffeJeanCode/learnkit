import { ThinkingSketch, WaitingHint } from "../../components/AgentLoader";
import type { GenerationStage } from "../../components/AgentLoader";
import { BlockDraftPreview } from "./BlockDraftPreview";
import { type BlockDraft, hasDraftContent } from "./blockDraft";
import type { SkeletonKind } from "./blockTypeInfo";
import { describeGeneration } from "./generationStatus";

/** Placeholder with the silhouette of the block that is about to land. Pure
 *  decoration (`aria-hidden`): the status line above it carries the meaning. */
function BlockSkeleton({ kind, compact }: { kind: SkeletonKind; compact: boolean }) {
  const lines = (widths: string[]) => widths.map((w, i) => <span key={i} className="sk-line" style={{ width: w }} />);
  return (
    <div className={`gen-skeleton gen-skeleton-${kind}${compact ? " compact" : ""}`} aria-hidden="true">
      {kind === "text" && (
        <>
          {!compact && <span className="sk-line sk-title" />}
          {lines(compact ? ["94%", "70%"] : ["100%", "92%", "97%", "68%"])}
        </>
      )}
      {kind === "choices" && (
        <>
          {lines(["88%"])}
          {!compact && <span className="sk-option" />}
          <span className="sk-option" />
          <span className="sk-option" />
        </>
      )}
      {kind === "diagram" && (
        <>
          <span className="sk-box sk-diagram" />
          {lines(["54%"])}
        </>
      )}
      {kind === "task" && (
        <>
          {lines(compact ? ["90%"] : ["100%", "84%"])}
          <span className="sk-box sk-input" />
        </>
      )}
    </div>
  );
}

/**
 * What the student sees while the next block is being made: a status line from
 * real signals (see `describeGeneration`), the text of the block as it streams
 * in when the server shares it, a skeleton shaped like the block, and the
 * stopwatch. Remount it per block (`key`) so the stopwatch restarts.
 *
 * Replaces the fixed "Preparando el siguiente paso…" row, which looked the same
 * whether the model was thinking, writing, or retrying.
 */
export function BlockGeneratingCard({
  stage,
  draft,
  onCancel,
}: {
  stage: GenerationStage;
  draft: BlockDraft | null;
  onCancel?: () => void;
}) {
  const status = describeGeneration(stage, draft);
  const hasText = hasDraftContent(draft) && Object.keys(draft.fields).length > 0;

  return (
    <div className={`gen-card phase-${status.phase}`}>
      <div className="gen-card-head" role="status" aria-live="polite">
        <ThinkingSketch label={status.headline} />
        {onCancel && (
          <button
            type="button"
            className="buffer-cancel"
            title="Cancelar y reintentar"
            aria-label="Cancelar y reintentar"
            onClick={onCancel}
          >
            ×
          </button>
        )}
      </div>
      {status.detail && <p className="gen-card-detail">{status.detail}</p>}
      {hasText && <BlockDraftPreview draft={draft} showKind={false} />}
      <BlockSkeleton kind={status.skeleton} compact={hasText} />
      <WaitingHint />
    </div>
  );
}
