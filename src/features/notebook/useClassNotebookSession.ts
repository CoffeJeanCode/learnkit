import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { GenerationStage } from "../../components/AgentLoader";
import type { GateResult, GateSubmission, NotebookBlock, NotebookBlockEvent, NotebookDocument } from "../../lib/schemas";
import { onAgentEvent, onNotebookBlockEvent, retryPendingBlock as retryPendingBlockCommand, startClassNotebook, submitGateResponse, tauriError } from "../../lib/tauri";

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
  const documentIdRef = useRef<string | null>(null);
  const generatorStartsRef = useRef(0);
  const blockBoundaryRef = useRef(false);

  useEffect(() => {
    documentIdRef.current = null;
    generatorStartsRef.current = 0;
    blockBoundaryRef.current = false;
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
              }
            },
          ],
          [
            "notebook_block://ready",
            (e) => {
              if (e.document_id !== documentIdRef.current || !e.block) return;
              setBlocks((prev) => upsertBlock(prev, e.block!));
              setBufferState("idle");
            },
          ],
          [
            "notebook_block://error",
            (e) => {
              if (e.document_id !== documentIdRef.current) return;
              setBufferState("error");
              setBufferError(e.message ?? "No se pudo generar el siguiente bloque.");
            },
          ],
          // Unfiltered: a `generating` boundary marks "a NEW block started",
          // which is how the stage listener below tells block N+1's chain
          // apart from a retry of THIS block (no boundary in between).
          // During loading any boundary is effectively this document's —
          // `documentIdRef` isn't set until the payload arrives.
          [
            "notebook_block://generating",
            () => {
              blockBoundaryRef.current = true;
            },
          ],
        ];
        const acquired = await Promise.all([
          ...subs.map(([channel, fn]) => onNotebookBlockEvent(channel, fn)),
          onAgentEvent("agent://started", (e) => {
            if (e.agent_id === "notebook_generator") {
              generatorStartsRef.current += 1;
              setStage(
                generatorStartsRef.current === 1 || blockBoundaryRef.current ? "escribiendo" : "reintentando",
              );
            } else if (e.agent_id === "pedagogical_critic") {
              setStage("revisando");
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

  const submitGate = async (blockId: string, submission: GateSubmission): Promise<GateResult> => {
    const result = await submitGateResponse(blockId, submission);
    setBlocks((prev) => {
      let next = upsertBlock(prev, result.block);
      if (result.escalation_block) next = upsertBlock(next, result.escalation_block);
      if (result.next_block) next = upsertBlock(next, result.next_block);
      return next;
    });
    return result;
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

  return { document, blocks, bufferState, bufferError, loading, error, stage, submitGate, retryPendingBlock };
}
