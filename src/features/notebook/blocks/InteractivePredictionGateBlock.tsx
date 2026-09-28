import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "interactive_prediction_gate" }>;

/** A hypothesis question that GATES the next block — the student commits to
 *  one option, which is graded server-side (`correctOption` never reaches
 *  this component). Wrong answers stay locked on this same block (up to 3
 *  attempts) with a scaffold hint, never the answer; the 3rd wrong answer
 *  escalates to a re-approach block instead. */
export function InteractivePredictionGateBlock({
  block,
  content,
  isActive,
  onSubmitGate,
}: {
  block: NotebookBlock;
  content: Content;
  isActive: boolean;
  onSubmitGate: SubmitGate;
}) {
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const choose = async (option: string) => {
    if (!isActive || busy || resolved) return;
    setSelected(option);
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "interactive_prediction_gate", selectedOption: option });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
    } finally {
      setBusy(false);
    }
  };

  if (resolved) {
    return (
      <div className="notebook-block-body">
        {content.visualAid && <StaticVisual visual={content.visualAid} />}
        <RichText className="block-question" text={content.question} />
        <p className="hint">Ya superaste esta predicción.</p>
      </div>
    );
  }

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);

  return (
    <div className="notebook-block-body">
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <RichText className="block-question" text={content.question} />
      <div className="prediction-options">
        {content.options.map((opt) => (
          <button key={opt} className={selected === opt ? "btn-primary" : ""} onClick={() => void choose(opt)} disabled={!isActive || busy}>
            <InlineText text={opt} />
          </button>
        ))}
      </div>
      {busy && <p className="hint">Calificando…</p>}
      {!busy && selected && content.conceptualFeedbackMap[selected] && (
        <p className="hint">
          <InlineText text={content.conceptualFeedbackMap[selected]} />
        </p>
      )}
      {!busy && outcome && !outcome.passed && (
        <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "No es correcto — inténtalo de nuevo."}</p>
      )}
    </div>
  );
}
