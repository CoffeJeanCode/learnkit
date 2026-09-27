import { useEffect, useRef, useState } from "react";
import { askLexicalAssistant, tauriError } from "../../lib/tauri";
import type { LexicalTurn } from "../../lib/tauri";

const MAX_HISTORY_TURNS = 4;

/** Small isolated assistant, one per block: a physical "¿Dudas?" button that
 *  opens an anchored dropdown (not a full-screen modal) scoped to THAT
 *  block's own content — `fragmentContext` and `answerBearingStrings` are
 *  derived from this specific block (see `blocks/index.tsx`), so the
 *  assistant actually has enough context to answer well. Answers in 3 layers
 *  (analogía / mecanismo / límite), never the answer to a gate. Keeps a
 *  short in-dropdown memory (last few Q&A) for "explícamelo de otra forma"
 *  follow-ups — cleared whenever it closes, never persisted server-side. See
 *  `LexicalAssistantService::ask` for the isolation + redaction contract. */
export function BlockLexicalAssistant({
  term,
  fragmentContext,
  keyConcepts,
  answerBearingStrings,
}: {
  term: string;
  fragmentContext: string | null;
  keyConcepts: string[];
  answerBearingStrings: string[];
}) {
  const [open, setOpen] = useState(false);
  const [question, setQuestion] = useState("");
  const [history, setHistory] = useState<LexicalTurn[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onOutsideClick = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onOutsideClick);
    return () => document.removeEventListener("mousedown", onOutsideClick);
  }, [open]);

  const ask = async () => {
    const q = question.trim();
    if (!q || busy) return;
    setBusy(true);
    setError(null);
    try {
      const answer = await askLexicalAssistant(
        term,
        fragmentContext,
        keyConcepts,
        history.slice(-MAX_HISTORY_TURNS),
        q,
        answerBearingStrings,
      );
      setHistory((h) => [...h, { question: q, answer }]);
      setQuestion("");
    } catch (e) {
      setError(tauriError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="block-lexical" ref={rootRef}>
      <button type="button" className="btn-quiet block-lexical-trigger" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        ¿Dudas?
      </button>
      {open && (
        <div className="block-lexical-dropdown card">
          <div className="lexical-popover-history">
            {history.length === 0 && !busy && (
              <p className="hint">Pregunta lo que necesites aclarar sobre este bloque — no te voy a dar la respuesta del ejercicio.</p>
            )}
            {history.map((turn, i) => (
              <div className="lexical-turn" key={i}>
                <p className="lexical-turn-question">Tú: {turn.question}</p>
                <p className="lexical-turn-answer">{turn.answer}</p>
              </div>
            ))}
            {busy && <p className="hint">Pensando…</p>}
          </div>

          {error && <div className="alert error">{error}</div>}

          <div className="row lexical-popover-input">
            <input
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void ask();
              }}
              placeholder="¿Qué quieres que te aclare?"
              disabled={busy}
              autoFocus
            />
            <button className="btn-primary" onClick={() => void ask()} disabled={busy || !question.trim()}>
              Preguntar
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
