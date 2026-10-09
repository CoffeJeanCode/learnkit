import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "branching_scenario_challenge" }>;

/** Situational decision-making with visible consequences per path — the
 *  student picks a branch, graded server-side against `isOptimal` (never
 *  sent to this component). A non-optimal pick still shows its own
 *  consequence, but locks the gate the same as any other failed attempt. */
export function BranchingScenarioChallengeBlock({
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
  const [chosen, setChosen] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const choose = async (choice: string) => {
    if (!isActive || busy || resolved) return;
    setChosen(choice);
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "branching_scenario_challenge", selectedChoice: choice });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
    } finally {
      setBusy(false);
    }
  };

  if (resolved) {
    return (
      <div className="notebook-block-body">
        <RichText className="block-question" text={content.scenario} />
        {content.visualAid && <StaticVisual visual={content.visualAid} />}
        <p className="hint">{block.status === "passed" ? "Ya resolviste esta decisión." : "Esta decisión no se resolvió todavía: continuaremos con otra estrategia."}</p>
      </div>
    );
  }

  const chosenBranch = content.branches.find((b) => b.choice === chosen);
  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);

  return (
    <div className="notebook-block-body">
      <RichText className="block-question" text={content.scenario} />
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <p className="hint">
        <InlineText text={content.decisionPoint} />
      </p>
      <div className="prediction-options">
        {content.branches.map((b) => (
          <button key={b.choice} className={chosen === b.choice ? "btn-primary" : ""} onClick={() => void choose(b.choice)} disabled={!isActive || busy}>
            <InlineText text={b.choice} />
          </button>
        ))}
      </div>
      {busy && <p className="hint">Calificando…</p>}
      {!busy && chosenBranch && (
        <p className="hint">
          <InlineText text={chosenBranch.consequence} />
        </p>
      )}
      {!busy && outcome && !outcome.passed && (
        <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "Esa no era la mejor opción — inténtalo de nuevo."}</p>
      )}
    </div>
  );
}
