import { useEffect, useRef, useState } from "react";

/** Saves a block's annotation: `note` (free text) and/or `selection` (UI state). */
export type Annotate = (blockId: string, patch: { note?: string; selection?: Record<string, unknown> }) => void;

const SAVE_DEBOUNCE_MS = 400;
const NOTE_DEBOUNCE_MS = 800;
export const MAX_NOTE_CHARS = 4000;

/** Defensive readers for state restored from storage: a stored value can come
 *  from an older shape, so each field is validated, never trusted. */
export const asStringArray = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
export const asNullableString = (v: unknown): string | null => (typeof v === "string" ? v : null);
export const asNumber = (v: unknown, fallback = 0): number => (typeof v === "number" && Number.isFinite(v) ? v : fallback);

/** Component state that survives a reload: restored from the block's saved
 *  `studentState` and written back (debounced) after every change. `fallback`
 *  supplies the defaults and the keys; stored values override it. Without
 *  `onAnnotate` it behaves like plain state. */
export function usePersistedState<T extends Record<string, unknown>>(
  blockId: string,
  saved: unknown,
  fallback: T,
  onAnnotate?: Annotate,
): [T, (patch: Partial<T>) => void] {
  const [state, setState] = useState<T>(() => ({
    ...fallback,
    ...(saved && typeof saved === "object" && !Array.isArray(saved) ? (saved as Partial<T>) : {}),
  }));
  const dirty = useRef(false);

  useEffect(() => {
    if (!dirty.current || !onAnnotate) return;
    const timer = setTimeout(() => onAnnotate(blockId, { selection: state }), SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
    // `onAnnotate` is a fresh closure every render; only the state drives saves.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state, blockId]);

  const update = (patch: Partial<T>) => {
    dirty.current = true;
    setState((s) => ({ ...s, ...patch }));
  };
  return [state, update];
}

/** "Mis notas": a private, collapsible scratchpad under a question. Autosaves
 *  while typing and on blur; never graded and never sent to the model. */
export function BlockNote({ blockId, saved, onAnnotate }: { blockId: string; saved?: string | null; onAnnotate?: Annotate }) {
  const [note, setNote] = useState(saved ?? "");
  const [status, setStatus] = useState<"idle" | "pending" | "saved">("idle");
  const lastSaved = useRef(saved ?? "");
  const latest = useRef(note);
  latest.current = note;

  const flush = () => {
    if (!onAnnotate || latest.current === lastSaved.current) return;
    lastSaved.current = latest.current;
    onAnnotate(blockId, { note: latest.current });
    setStatus("saved");
  };

  useEffect(() => {
    if (!onAnnotate || note === lastSaved.current) return;
    setStatus("pending");
    const timer = setTimeout(flush, NOTE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [note]);

  // Flush a pending edit if the block unmounts before the debounce fires.
  useEffect(() => flush, []); // eslint-disable-line react-hooks/exhaustive-deps

  if (!onAnnotate) return null;
  return (
    <details className="block-note" open={note.trim().length > 0}>
      <summary>
        📝 Mis notas{note.trim() && <span className="block-note-dot" aria-label="Tienes una nota guardada"> ●</span>}
      </summary>
      <textarea
        value={note}
        maxLength={MAX_NOTE_CHARS}
        rows={3}
        placeholder="Anota lo que quieras recordar de esta pregunta (es privado: no se califica)…"
        onChange={(e) => setNote(e.target.value)}
        onBlur={flush}
        aria-label="Mis notas sobre esta pregunta"
      />
      <span className="hint block-note-status" aria-live="polite">
        {status === "pending" ? "Guardando…" : status === "saved" ? "Guardado" : ""}
      </span>
    </details>
  );
}
