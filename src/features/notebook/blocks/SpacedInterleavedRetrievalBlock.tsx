import { useState } from "react";
import type { DynamicSectionBlock } from "../../../lib/schemas";
import { InlineText } from "./RichText";

type Content = Extract<DynamicSectionBlock, { blockType: "spaced_interleaved_retrieval" }>;

// One reactivated concept: the student recalls FIRST, then reveals the
// expected answer — the testing-effect interaction (Bjork). Non-gate: there
// is nothing to submit or grade here, `expectedAnswer` is shown outright by
// the backend (see `RetrievalPromptSchema`'s doc comment), this component
// just delays it behind a click so recall still happens before comparison.
function RetrievalItem({ conceptLabel, prompt, expectedAnswer }: { conceptLabel: string; prompt: string; expectedAnswer: string }) {
  const [revealed, setRevealed] = useState(false);
  return (
    <li className="retrieval-item">
      <span className="retrieval-concept-tag">
        <InlineText text={conceptLabel} />
      </span>
      <p className="retrieval-prompt">
        <InlineText text={prompt} />
      </p>
      {revealed ? (
        <p className="retrieval-answer">
          <InlineText text={expectedAnswer} />
        </p>
      ) : (
        <button className="btn-quiet" onClick={() => setRevealed(true)}>
          Ya recordé — mostrar respuesta
        </button>
      )}
    </li>
  );
}

// Quick recall of 1-2 concepts from PRIOR classes — never this class's new
// material. Only ever the class's FIRST block, and only when the backend's
// `learnerMemory.dueRetrieval` had items (see `domain::learner_memory` in
// Rust): spaced retrieval before touching anything new.
export function SpacedInterleavedRetrievalBlock({ content }: { content: Content }) {
  return (
    <div className="notebook-block-body retrieval-recap">
      <p className="hint">Antes de seguir: repasa rápido lo que ya viste.</p>
      <ul className="retrieval-list">
        {content.items.map((item, i) => (
          <RetrievalItem key={i} conceptLabel={item.conceptLabel} prompt={item.prompt} expectedAnswer={item.expectedAnswer} />
        ))}
      </ul>
    </div>
  );
}
