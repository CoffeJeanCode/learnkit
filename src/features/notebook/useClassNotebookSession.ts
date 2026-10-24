import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { GenerationStage } from "../../components/AgentLoader";
import type { GateResult, GateSubmission, NotebookBlock, NotebookBlockEvent, NotebookDocument } from "../../lib/schemas";
import { onAgentEvent, onNotebookBlockEvent, regenerateNotebookBlock, requestEasierStep as requestEasierStepCommand, retryPendingBlock as retryPendingBlockCommand, saveBlockAnnotation, startClassNotebook, submitGateResponse, tauriError } from "../../lib/tauri";
import { type BlockDraft, applyBlockDelta } from "./blockDraft";

export type BufferState = "idle" | "generating" | "error";

function sortByOrder(blocks: NotebookBlock[]): NotebookBlock[] {
  return [...blocks].sort((a, b) => a.order_index - b.order_index);
}

function upsertBlock(blocks: NotebookBlock[], next: NotebookBlock): NotebookBlock[] {
  const exists = blocks.some((b) => b.id === next.id);
  return sortByOrder(exists ? blocks.map((b) => (b.id === next.id ? next : b)) : [...blocks, next]);
}

/**
 * Drives one class's iterative, mastery-gated notebook: starts (or resumes)
 * it, tracks the buffer state for the block the backend is generating in
 * the background, and applies gate submissions. Only blocks the backend has
 * actually persisted ever appear in `blocks` — a future, not-yet-generated
 * block is never fabricated client-side.
 */
export function useClassNotebookSession(classId: string | null, reloadToken = 0) {
  const [document, setDocument] = useState<NotebookDocument | null>(null);
  const [blocks, setBlocks] = useState<NotebookBlock[]>([]);
  const [bufferState, setBufferState] = useState<BufferState>("idle");
  const [bufferError, setBufferError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Real stage of the FIRST generation, driven by the backend's `agent://*`
  // events (generator started → escribiendo, critic → revisando, a second
  // generator run without a block boundary → reintentando) so the loader
  // narrates what the model is actually doing instead of looping one line.
  const [stage, setStage] = useState<GenerationStage>("preparando");
  const [regeneratingBlockId, setRegeneratingBlockId] = useState<string | null>(null);
  // Live preview of the block being written (from `notebook_block://delta`).
  // Cleared the moment the real block arrives, a new block starts, or it fails.
  const [draft, setDraft] = useState<BlockDraft | null>(null);
  const documentIdRef = useRef<string | null>(null);
  // Generator runs started for the block being made RIGHT NOW: 1 = first try,
  // 2+ = a retry. Reset at every block boundary (`generating`).
  const generatorStartsRef = useRef(0);

  useEffect(() => {
    documentIdRef.current = null;
    generatorStartsRef.current = 0;
    if (!classId) {
      setDocument(null);
      setBlocks([]);
      return;
    }
    let cancelled = false;
    let unlisteners: UnlistenFn[] = [];
    setLoading(true);
    setError(null);
    setBufferError(null);
    setBufferState("idle");
    setStage("preparando");
    setDraft(null);
    setDocument(null);
    setBlocks([]);

    (async () => {
      // Everything below — event subscription AND the actual start call —
      // is inside ONE try/finally: an exception from either (e.g. `listen`
      // rejecting before Tauri's IPC bridge is ready) must still clear
      // `loading` and surface as `error`, never leave the student staring
      // at the generation loader forever with no way to retry.
      try {
        const subs: Array<[Parameters<typeof onNotebookBlockEvent>[0], (e: NotebookBlockEvent) => void]> = [
          [
            "notebook_block://generating",
            (e) => {
              if (e.document_id === documentIdRef.current) {
                setBufferState("generating");
                setBufferError(null);
                // A new block starts: drop the previous block's draft so its
                // attempt counter cannot make the new one look stale.
                setDraft(null);
              }
            },
          ],
          [
            "notebook_block://ready",
            (e) => {
              if (e.document_id !== documentIdRef.current || !e.block) return;
              setBlocks((prev) => upsertBlock(prev, e.block!));
              setBufferState("idle");
              setDraft(null);
            },
          ],
          [
            "notebook_block://error",
            (e) => {
              if (e.document_id !== documentIdRef.current) return;
              setBufferState("error");
              setBufferError(e.message ?? "No se pudo generar el siguiente bloque.");
              setDraft(null);
            },
          ],
          // Live progress. Unlike the events above it must also be accepted
          // WHILE LOADING: block 1 is generated inside `start_class_notebook`,
          // before `documentIdRef` is known, so any delta then is this
          // document's (same reasoning as the unfiltered `generating` below).
          [
            "notebook_block://delta",
            (e) => {
              if (documentIdRef.current !== null && e.document_id !== documentIdRef.current) return;
              setDraft((prev) => applyBlockDelta(prev, e));
            },
          ],
          // Unfiltered: a `generating` boundary marks "a NEW block started" —
          // the attempt count and stage start over for it. A second generator
          // run WITHOUT a boundary in between is a retry of the SAME block.
          // During loading any boundary is effectively this document's —
          // `documentIdRef` isn't set until the payload arrives.
          [
            "notebook_block://generating",
            () => {
              generatorStartsRef.current = 0;
              setStage("preparando");
            },
          ],
        ];
        const acquired = await Promise.all([
          ...subs.map(([channel, fn]) => onNotebookBlockEvent(channel, fn)),
          onAgentEvent("agent://started", (e) => {
            // Only the generator narrates the stage. The pedagogical critic
            // now runs AFTER a block is shown, in parallel with the next one,
            // so its start says nothing about the block being written.
            if (e.agent_id === "notebook_generator") {
              generatorStartsRef.current += 1;
              setStage(generatorStartsRef.current === 1 ? "escribiendo" : "reintentando");
            }
          }),
        ]);
        if (cancelled) {
          acquired.forEach((u) => u());
        } else {
          unlisteners = acquired;
        }

        const payload = await startClassNotebook(classId);
        if (cancelled) return;
        documentIdRef.current = payload.document.id;
        setDocument(payload.document);
        setBlocks(sortByOrder(payload.blocks));
        // Block 1 is in `payload`, not in a `ready` event, so its draft is
        // cleared here (the next block's `generating` clears again).
        setDraft(null);
      } catch (e) {
        console.error("useClassNotebookSession: start_class_notebook failed", e);
        if (!cancelled) setError(tauriError(e).message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      unlisteners.forEach((u) => u());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [classId, reloadToken]);

  const applyGateResult = (result: GateResult) => {
    setBlocks((prev) => {
      let next = upsertBlock(prev, result.block);
      if (result.escalation_block) next = upsertBlock(next, result.escalation_block);
      if (result.next_block) next = upsertBlock(next, result.next_block);
      return next;
    });
  };

  const submitGate = async (blockId: string, submission: GateSubmission): Promise<GateResult> => {
    const result = await submitGateResponse(blockId, submission);
    applyGateResult(result);
    return result;
  };

  /** Persists the student's note / selection for one block and mirrors it into
   *  local state, so a remount (or a re-render from a gate result) already has
   *  it. Failures are logged, never thrown: losing an annotation must not
   *  interrupt the lesson. */
  const annotateBlock = (blockId: string, patch: { note?: string; selection?: Record<string, unknown> }) => {
    const documentId = documentIdRef.current;
    if (!documentId) return;
    setBlocks((prev) =>
      prev.map((b) =>
        b.id === blockId
          ? {
              ...b,
              content_json: {
                ...b.content_json,
                ...(patch.note !== undefined ? { studentNote: patch.note } : {}),
                ...(patch.selection !== undefined ? { studentState: patch.selection } : {}),
              },
            }
          : b,
      ),
    );
    saveBlockAnnotation(documentId, blockId, patch).catch((e) => console.error("save_block_annotation failed", e));
  };

  /** Voluntary de-escalation: the student asks for a more guided step instead
   *  of failing the gate 3 times. Returns the locked gate + the new block. */
  const requestEasierStep = async (blockId: string, hintsShown: number, draft: string): Promise<GateResult> => {
    const result = await requestEasierStepCommand(blockId, hintsShown, draft);
    applyGateResult(result);
    return result;
  };

  /** Repairs a block whose stored JSON the renderer can't display anymore:
   *  the backend regenerates it IN PLACE (same id, position and block type)
   *  and returns the whole class, which replaces local state wholesale —
   *  the only reliable refresh, since the block's id never changes. Throws
   *  the backend's message on failure so the caller can show it on the card.
   */
  const regenerateBlock = async (blockId: string) => {
    setRegeneratingBlockId(blockId);
    try {
      const payload = await regenerateNotebookBlock(blockId);
      setDocument(payload.document);
      setBlocks(sortByOrder(payload.blocks));
    } catch (e) {
      throw new Error(tauriError(e).message);
    } finally {
      setRegeneratingBlockId(null);
    }
  };

  const retryPendingBlock = async () => {
    const documentId = documentIdRef.current;
    if (!documentId) return;
    setBufferState("generating");
    setBufferError(null);
    try {
      await retryPendingBlockCommand(documentId);
    } catch (e) {
      setBufferState("error");
      setBufferError(tauriError(e).message);
    }
  };

  /** Never calls the backend: a hung generation is abandoned in place, not
   *  cancelled. It only flips local state into the existing error branch, so
   *  the student's "Reintentar" reuses `retryPendingBlock`'s already-safe
   *  path — `insert_block_if_count` discards the abandoned generation's
   *  late insert as having "lost the race" once the retry's own lands.
   */
  const cancelBuffer = () => {
    setBufferState("error");
    setBufferError("Cancelado. Toca «Reintentar» para seguir con la clase.");
    setDraft(null);
  };

  return {
    document,
    blocks,
    bufferState,
    bufferError,
    loading,
    error,
    stage,
    draft,
    submitGate,
    requestEasierStep,
    annotateBlock,
    retryPendingBlock,
    cancelBuffer,
    regenerateBlock,
    regeneratingBlockId,
  };
}
