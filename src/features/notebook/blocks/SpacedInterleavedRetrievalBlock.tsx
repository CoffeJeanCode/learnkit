import { useState } from "react";
import type { DynamicSectionBlock } from "../../../lib/schemas";
import { revealRetrievalAnswer, submitRetrievalAnswer } from "../../../lib/tauri";
import { InlineText } from "./RichText";

type Content = Extract<DynamicSectionBlock, { blockType: "spaced_interleaved_retrieval" }>;

type Outcome = "correct" | "incorrect" | "self_recalled" | "self_forgot";

// One reactivated concept. The student WRITES their answer from memory; only
// then does the backend grade it and reveal the solution (the client never has
// it before — see `redact_block` in Rust). That is what makes a correct answer
// evidence of recall. Giving up is allowed ("No lo recuerdo") and is recorded
// as a miss, without penalty.
function RetrievalItem({ blockId, index, item }: { blockId: string; index: number; item: Content["items"][number] }) {
  const [answer, setAnswer] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(item.reportedOutcome ?? null);
  const [expected, setExpected] = useState<string | null>(item.expectedAnswer ?? null);

  const check = async () => {
    if (busy || !answer.trim()) return;
    setBusy(true);
    setError(false);
    try {
      const r = await submitRetrievalAnswer(blockId, index, answer.trim());
      setExpected(r.expectedAnswer);
      setOutcome(r.correct ? "correct" : "incorrect");
    } catch {
      setError(true);
    } finally {
      setBusy(false);
    }
  };

  const giveUp = async () => {
    if (busy) return;
    setBusy(true);
    setError(false);
    try {
      setExpected(await revealRetrievalAnswer(blockId, index));
      setOutcome("self_forgot");
    } catch {
      setError(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="retrieval-item">
      <span className="retrieval-concept-tag">
        <InlineText text={item.conceptLabel} />
      </span>
      <p className="retrieval-prompt">
        <InlineText text={item.prompt} />
      </p>
      {outcome === null ? (
        <>
          <textarea
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            placeholder="Responde de memoria, con tus palabras…"
            rows={2}
            disabled={busy}
          />
          <div className="row">
            <button className="btn-primary" onClick={() => void check()} disabled={busy || !answer.trim()}>
              {busy ? "Comprobando…" : "Comprobar"}
            </button>
            <button className="btn-quiet" onClick={() => void giveUp()} disabled={busy}>
              No lo recuerdo
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="hint">
            {outcome === "correct"
              ? "Lo recordaste."
              : "Todavía no — sin penalización, lo repasaremos de nuevo más adelante."}
          </p>
          {expected && (
            <p className="retrieval-answer">
              <InlineText text={expected} />
            </p>
          )}
        </>
      )}
      {error && <p className="hint">No se pudo completar. Inténtalo de nuevo.</p>}
    </li>
  );
}

// Quick recall of 1-2 concepts from PRIOR classes — never this class's new
// material. Only ever the class's FIRST block, and only when the backend's
// `learnerMemory.dueRetrieval` had items (see `domain::learner_memory` in
// Rust): spaced retrieval before touching anything new.
export function SpacedInterleavedRetrievalBlock({ blockId, content }: { blockId: string; content: Content }) {
  return (
    <div className="notebook-block-body retrieval-recap">
      <p className="hint">Antes de seguir: repasa rápido lo que ya viste.</p>
      <ul className="retrieval-list">
        {content.items.map((item, i) => (
          <RetrievalItem key={i} blockId={blockId} index={i} item={item} />
        ))}
      </ul>
    </div>
  );
}
