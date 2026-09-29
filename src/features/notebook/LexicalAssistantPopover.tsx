import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode, RefObject } from "react";
import { askLexicalAssistant, tauriError } from "../../lib/tauri";
import type { LexicalTurn } from "../../lib/tauri";
import { useMarginCommentPosition, useTextSelectionPopover, type SelectionAnchor } from "./useTextSelectionPopover";
import { RichText } from "./blocks/RichText";

const MAX_HISTORY_TURNS = 4;

/** The per-block "¿Dudas?" TRIGGER: a physical button in the block's kicker
 *  row that opens THIS block's shared margin comment (owned by
 *  `TheorySelectionAssistant` below — one comment slot per block, two
 *  triggers, design §1). It deliberately carries no ask/history state of its
 *  own: the thread lives in `useLexicalAssistantThread` inside the unified
 *  surface, so the request/history/error contract exists in exactly one
 *  place. The block-scoped context (`fragmentContext` /
 *  `answerBearingStrings`, from `deriveBlockLexicalContext()` in
 *  `blocks/index.tsx`) is fed straight into that comment instead. Answers
 *  stay in 3 layers (analogía / mecanismo / límite), never the answer to a
 *  gate. The thread is cleared when the comment closes UNLESS the student
 *  already got an answer for that fragment — then the turns are snapshotted
 *  into `TheorySelectionAssistant`'s `savedThreadRef` and the highlight stays,
 *  so clicking the mark brings the help back (never persisted server-side; see
 *  `LexicalAssistantService::ask` for the isolation + redaction contract). */
export function BlockLexicalAssistant({
  open,
  onToggle,
  triggerRef,
}: {
  /** Whether this block's comment slot is currently open via the button. */
  open: boolean;
  onToggle: () => void;
  /** Shared with the comment: mousedown on the button must TOGGLE (not be
   *  treated as an outside click), and the button rect anchors the
   *  narrow-viewport centering fallback. */
  triggerRef: RefObject<HTMLButtonElement | null>;
}) {
  return (
    <div className="block-lexical">
      <button
        type="button"
        className="btn-quiet block-lexical-trigger"
        ref={triggerRef}
        onClick={onToggle}
        aria-expanded={open}
      >
        ¿Dudas?
      </button>
    </div>
  );
}

// --- The unified margin comment (both triggers) -----------------------------
//
// ONE comment surface per block with two ways to open it: the ¿Dudas?
// button above (button mode) and a floating comment for whatever the
// student highlights inside the block (selection mode, see
// `useTextSelectionPopover`). Both render `LexicalMarginComment`, which
// keeps the request/history/busy/error state machine in
// `useLexicalAssistantThread` below, so neither trigger reimplements the
// `askLexicalAssistant` call, history trimming, or error handling.

/** One isolated lexical-assistant thread: question/history/busy/error state
 *  plus the `ask()` call itself, used by the unified margin comment for
 *  BOTH triggers (button and selection), so the request/history/error
 *  contract lives in exactly one place. */
function useLexicalAssistantThread(params: {
  term: string;
  fragmentContext: string | null;
  keyConcepts: string[];
  answerBearingStrings: string[];
  /** Turns the comment remounts with — how an already-answered fragment gets
   *  its help back after the popover closed over it (see `savedThreadRef` in
   *  `TheorySelectionAssistant`). */
  initialHistory?: LexicalTurn[];
  /** Reports every history change so the owner can snapshot it BEFORE the
   *  comment unmounts: closing the comment is otherwise the moment the turns
   *  would be lost, and they have to outlive it to be reviewable. */
  onHistoryChange?: (history: LexicalTurn[]) => void;
}) {
  const { term, fragmentContext, keyConcepts, answerBearingStrings, initialHistory, onHistoryChange } = params;
  const [question, setQuestion] = useState("");
  const [history, setHistory] = useState<LexicalTurn[]>(() => initialHistory ?? []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onHistoryChange?.(history);
  }, [history, onHistoryChange]);

  const ask = async (explicitQuestion?: string) => {
    const q = (explicitQuestion ?? question).trim();
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

  return { question, setQuestion, history, busy, error, ask };
}

const SELECTION_TERM_MAX_CHARS = 240;
const SELECTION_QUOTE_PREVIEW_CHARS = 160;

function previewQuote(text: string, max = SELECTION_QUOTE_PREVIEW_CHARS): string {
  return text.length > max ? `${text.slice(0, max).trimEnd()}…` : text;
}

/** Owns the ONE margin-comment slot per block (design §1): wraps the
 *  block's rendered content, watches for a real text selection inside it
 *  (via `useTextSelectionPopover`), and renders the shared
 *  `LexicalMarginComment` whenever either trigger opens it — a live
 *  selection (which always wins) or the block's ¿Dudas? button (whose open
 *  state arrives as props from `blocks/index.tsx`, the components' common
 *  parent). Mounted for every block type; its context props come from the
 *  exhaustive `deriveBlockLexicalContext()` in `blocks/index.tsx`. */
export function TheorySelectionAssistant({
  blockRef,
  triggerRef,
  buttonOpen,
  onButtonOpenChange,
  buttonTerm,
  fragmentContext,
  keyConcepts,
  answerBearingStrings,
  children,
}: {
  /** `section.notebook-block-wrapper` — anchors the margin slot (`right`)
   *  and the button mode's `y` (block top). */
  blockRef: RefObject<HTMLElement | null>;
  /** The ¿Dudas? button — outside-click exemption + narrow fallback. */
  triggerRef: RefObject<HTMLButtonElement | null>;
  buttonOpen: boolean;
  onButtonOpenChange: (open: boolean) => void;
  /** Block-scoped term used when the button opens the slot (the selection
   *  mode derives its own term from the selected text). */
  buttonTerm: string;
  fragmentContext: string | null;
  keyConcepts: string[];
  answerBearingStrings: string[];
  children: ReactNode;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Snapshot of the thread belonging to the fragment the student last got
  // help with. It has to live HERE, above `LexicalMarginComment`: the comment
  // unmounts on close (its `key` is the selected text), so its own state dies
  // with it, and the whole point of keeping the highlight is that the answer
  // is still readable when the student clicks the mark again.
  const savedThreadRef = useRef<{ text: string; history: LexicalTurn[] } | null>(null);
  const savedThread = savedThreadRef.current;
  const { selection, popoverRef, clear } = useTextSelectionPopover(containerRef, {
    // Keep the mark only for a fragment that actually produced an answer —
    // a selection the student never asked about closes without leaving a
    // misleading "help is here" highlight behind. Reads the REF, not a
    // captured copy: the history lands in that ref from a child effect, which
    // never re-renders the parent, so a captured value would be stale for
    // exactly the close this rule exists to protect.
    keepOnClose: () => {
      const thread = savedThreadRef.current;
      return selection !== null && thread !== null && thread.text === selection.text && thread.history.length > 0;
    },
  });
  // Same text ⇒ same thread: the saved turns seed the remounting comment.
  // Safe to snapshot here — `initialHistory` is only ever consumed on the
  // mount triggered by a selection change, and that change re-renders us.
  const initialHistory = selection !== null && savedThread !== null && savedThread.text === selection.text ? savedThread.history : [];
  const onHistoryChange = useCallback((turns: LexicalTurn[]) => {
    const text = selection?.text;
    if (text) savedThreadRef.current = { text, history: turns };
  }, [selection]);

  // The comment is a DOM SIBLING of the tracked content, not a child of
  // `containerRef` — selecting text inside the comment itself (its quote
  // preview, a past answer) must NOT re-trigger detection against its own
  // rendered output, which would remount/destroy the thread mid-selection.
  //
  // ONE slot, two contexts: a live selection always wins over an open
  // button thread (switching resets the thread — design §5, matching today,
  // where the selection popover effectively took over the screen). `key`
  // preserves the old semantics exactly: a NEW distinct selection remounts
  // the comment (fresh thread); the button thread lives under a stable key
  // for as long as its slot is open, and unmounts when it closes (cleared
  // on close, as before).
  const closeSelection = clear;
  const closeButton = () => onButtonOpenChange(false);
  const comment =
    selection || buttonOpen ? (
      <LexicalMarginComment
        key={selection ? `selection:${selection.text}` : "button"}
        selectedText={selection?.text ?? null}
        anchor={selection?.anchor ?? null}
        buttonTerm={buttonTerm}
        blockRef={blockRef}
        triggerRef={triggerRef}
        fragmentContext={fragmentContext}
        keyConcepts={keyConcepts}
        answerBearingStrings={answerBearingStrings}
        rootRef={popoverRef}
        initialHistory={initialHistory}
        onHistoryChange={onHistoryChange}
        onClose={selection ? closeSelection : closeButton}
      />
    ) : null;

  return (
    <>
      <div className="theory-selection-zone" ref={containerRef}>
        {children}
      </div>
      {comment}
    </>
  );
}

/** The unified per-block margin comment: quoted fragment preview + a quick
 *  "Explicar término" action (selection mode) or the block-scoped hint
 *  (button mode), the shared history/input from `useLexicalAssistantThread`,
 *  and `position: fixed` coordinates beside its block computed by
 *  `useMarginCommentPosition` (margin slot right of the block, near-selection
 *  fallback, viewport-clamped, re-anchored on scroll/resize/content growth).
 *  Mounted only while its trigger is open — a NEW distinct selection remounts
 *  it via `key`, resetting the thread. */
function LexicalMarginComment({
  selectedText,
  anchor,
  buttonTerm,
  fragmentContext,
  keyConcepts,
  answerBearingStrings,
  blockRef,
  triggerRef,
  rootRef,
  initialHistory,
  onHistoryChange,
  onClose,
}: {
  selectedText: string | null;
  /** Viewport anchor of the selection, or `null` in button mode. */
  anchor: SelectionAnchor | null;
  buttonTerm: string;
  fragmentContext: string | null;
  keyConcepts: string[];
  answerBearingStrings: string[];
  blockRef: RefObject<HTMLElement | null>;
  triggerRef: RefObject<HTMLButtonElement | null>;
  rootRef: RefObject<HTMLDivElement | null>;
  /** Turns this thread starts from — non-empty only when the comment is
   *  remounting an already-answered fragment. */
  initialHistory: LexicalTurn[];
  /** Feeds the owner's `savedThreadRef` so the turns outlive this mount. */
  onHistoryChange: (history: LexicalTurn[]) => void;
  onClose: () => void;
}) {
  const isSelection = selectedText !== null;
  const term = selectedText !== null ? selectedText.slice(0, SELECTION_TERM_MAX_CHARS) : buttonTerm;
  const { question, setQuestion, history, busy, error, ask } = useLexicalAssistantThread({
    term,
    fragmentContext,
    keyConcepts,
    answerBearingStrings,
    initialHistory,
    onHistoryChange,
  });
  const { left, top, ready } = useMarginCommentPosition({ blockRef, triggerRef, popoverRef: rootRef, anchor });
  const historyRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Button mode closes on a mousedown outside the comment — except on its
  // own trigger, so the button's click still TOGGLES it (a close on mousedown
  // would flip the toggle back open one event later). Selection mode keeps
  // the `useTextSelectionPopover` outside-click/Escape rules instead; a
  // mousedown inside the content zone starts a fresh selection, which the
  // `selectionchange` handler drives either way.
  useEffect(() => {
    if (isSelection) return;
    const onOutsideClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      onClose();
    };
    document.addEventListener("mousedown", onOutsideClick);
    return () => document.removeEventListener("mousedown", onOutsideClick);
  }, [isSelection, rootRef, triggerRef, onClose]);

  // Focus the input once the comment is positioned and visible: button mode
  // autofocused before, but the unified surface stays `visibility: hidden`
  // until the first measure, which makes mount-time `autoFocus` a no-op.
  useEffect(() => {
    if (!isSelection && ready) inputRef.current?.focus();
  }, [isSelection, ready]);

  // Keep the newest turn (or the "Pensando…" row) visible: the history is a
  // scroll box, so without this an answer renders below the fold and looks
  // like it never arrived.
  useEffect(() => {
    const el = historyRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [history, busy]);

  return (
    <div className={`margin-comment card${ready ? " ready" : ""}`} style={{ left, top }} ref={rootRef}>
      {isSelection && (
        <button type="button" className="btn-quiet selection-popover-close" onClick={onClose} aria-label="Cerrar">
          ×
        </button>
      )}
      {selectedText !== null && (
        <p className="selection-quote">
          {'> "'}
          {previewQuote(selectedText)}
          {'"'}
        </p>
      )}

      <div className="lexical-popover-history" ref={historyRef}>
        {history.length === 0 && !busy && isSelection && (
          <>
            <p className="hint">Pregunta lo que quieras sobre esto — no te voy a dar la respuesta del ejercicio.</p>
            <div className="row">
              <button
                type="button"
                className="btn-quiet"
                onClick={() => void ask("Explícame qué significa esto.")}
                disabled={busy}
              >
                Explicar término
              </button>
            </div>
          </>
        )}
        {history.length === 0 && !busy && !isSelection && (
          <p className="hint">Pregunta lo que necesites aclarar sobre este bloque — no te voy a dar la respuesta del ejercicio.</p>
        )}
        {history.map((turn, i) => (
          <div className="lexical-turn" key={i}>
            <p className="lexical-turn-question">Tú: {turn.question}</p>
            <RichText text={turn.answer} className="lexical-turn-answer" />
          </div>
        ))}
        {busy && (
          <p className="hint lexical-thinking">
            <svg className="lex-lemniscate" viewBox="0 0 24 24" aria-hidden="true">
              <path
                className="lex-inf"
                pathLength={1}
                d="M12 12c-2-2.67-4-4-6-4a4 4 0 1 0 0 8c2 0 4-1.33 6-4Zm0 0c2 2.67 4 4 6 4a4 4 0 0 0 0-8c-2 0-4 1.33-6 4Z"
              />
            </svg>
            Pensando…
          </p>
        )}
      </div>

      {error && <div className="alert error">{error}</div>}

      <div className="row lexical-popover-input">
        <input
          ref={inputRef}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void ask();
          }}
          placeholder="¿Qué quieres que te aclare?"
          disabled={busy}
        />
        <button className="btn-primary" onClick={() => void ask()} disabled={busy || !question.trim()}>
          Preguntar
        </button>
      </div>
    </div>
  );
}
