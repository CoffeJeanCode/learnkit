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
 * What the student sees while the next block is being made:
 * If there is draft text already streamed, it appears in its own preview card
 * outside the loader block. The loader itself (status headline, skeleton, stopwatch)
 * appears in a separate card below it.
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
    <>
      {hasText && (
        <div className="card block-draft-card" aria-label="Adelanto del bloque en redacción">
          <div className="block-draft-card-head">
            <span className="block-draft-badge">
              <span className="dot dot-live" />
              Adelanto de contenido
            </span>
          </div>
          <BlockDraftPreview draft={draft} showKind={false} />
        </div>
      )}
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
        <BlockSkeleton kind={status.skeleton} compact={hasText} />
        <WaitingHint />
      </div>
    </>
  );
}
