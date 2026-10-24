import type { NotebookBlockEvent } from "../../lib/schemas";

/**
 * Live draft of the block the backend is writing right now, built from
 * `notebook_block://delta` events. It is a PREVIEW only: sanitized text the
 * server chose to show (block type + reading-only prose, never answer keys),
 * replaced by the real, validated block when `ready` arrives.
 */
export interface BlockDraft {
  /** Generator attempt this draft belongs to; a higher number = the previous
   *  draft was rejected and a new one started. */
  attempt: number;
  blockType: string | null;
  fields: Record<string, string>;
  /** Characters the model has reasoned so far (an amount, never the text). */
  reasoningChars: number;
}

const EMPTY: Omit<BlockDraft, "attempt"> = { blockType: null, fields: {}, reasoningChars: 0 };

/**
 * Folds one `delta` event into the draft. Pure.
 * - A newer attempt starts a fresh draft; a stale (lower) one is ignored.
 * - Preview fields replace the previous ones: the server resends the whole
 *   sanitized preview each time, so the draft only ever grows or stays.
 * - A preview with no known block type yet never erases a type we already have.
 */
export function applyBlockDelta(prev: BlockDraft | null, e: NotebookBlockEvent): BlockDraft | null {
  if (e.event !== "delta") return prev;
  const attempt = e.attempt ?? 1;
  if (prev && attempt < prev.attempt) return prev;
  const base: BlockDraft = !prev || attempt > prev.attempt ? { attempt, ...EMPTY } : prev;
  const next: BlockDraft = { ...base };
  if (e.preview) {
    next.blockType = e.preview.blockType ?? base.blockType;
    next.fields = e.preview.fields;
  }
  if (e.reasoning_chars != null) {
    next.reasoningChars = Math.max(base.reasoningChars, e.reasoning_chars);
  }
  return next;
}

/** Whether there is anything worth drawing yet. */
export function hasDraftContent(draft: BlockDraft | null): draft is BlockDraft {
  return draft !== null && (draft.blockType !== null || Object.keys(draft.fields).length > 0 || draft.reasoningChars > 0);
}
