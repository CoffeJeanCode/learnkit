import { useState } from "react";
import type { DynamicSectionBlock } from "../../../lib/schemas";
import { recordRetrievalResult } from "../../../lib/tauri";
import { InlineText } from "./RichText";

type Content = Extract<DynamicSectionBlock, { blockType: "spaced_interleaved_retrieval" }>;

// One reactivated concept: the student recalls FIRST, reveals the expected
// answer, then says honestly whether they recalled it. That self-report is the
// ONLY thing that moves their retrieval mastery — merely seeing this block
// does not (and self-reports count for less than a graded answer).
function RetrievalItem({
  blockId,
  index,
  item,
}: {
  blockId: string;
  index: number;
  item: Content["items"][number];
}) {
  const [revealed, setRevealed] = useState(false);
  const [reported, setReported] = useState<"self_recalled" | "self_forgot" | null>(item.reportedOutcome ?? null);
  const [error, setError] = useState(false);

  const report = async (recalled: boolean) => {
    setError(false);
    try {
      await recordRetrievalResult(blockId, index, recalled);
      setReported(recalled ? "self_recalled" : "self_forgot");
    } catch {
      setError(true);
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
      {revealed ? (
        <>
          <p className="retrieval-answer">
            <InlineText text={item.expectedAnswer} />
          </p>
          {reported ? (
            <p className="hint">
              {reported === "self_recalled"
                ? "Anotado: lo recordabas. Te lo volveremos a preguntar más adelante."
                : "Anotado: lo repasaremos pronto, sin penalización."}
            </p>
          ) : (
            <div className="row">
              <button className="btn-quiet" onClick={() => void report(true)}>
                Lo recordaba
              </button>
              <button className="btn-quiet" onClick={() => void report(false)}>
                No lo recordaba
              </button>
            </div>
          )}
          {error && <p className="hint">No se pudo guardar tu respuesta. Inténtalo de nuevo.</p>}
        </>
      ) : (
        <button className="btn-quiet" onClick={() => setRevealed(true)}>
          Ya intenté recordarlo — mostrar respuesta
        </button>
      )}
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
