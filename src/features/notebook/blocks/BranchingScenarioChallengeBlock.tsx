import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { asNullableString, asStringArray, usePersistedState } from "./persist";
import type { Annotate } from "./persist";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "branching_scenario_challenge" }>;

/** Situational decision-making with visible consequences per path — the
 *  student picks a branch, graded server-side against `isOptimal` (never
 *  sent to this component). A non-optimal pick shows its own consequence and
 *  is discarded: it stays visible but can't be picked again, so the optimum
 *  can't be found by clicking through every branch. The failed gate still
 *  locks like any other failed attempt. */
export function BranchingScenarioChallengeBlock({
  block,
  content,
  isActive,
  onSubmitGate,
  onAnnotate,
}: {
  block: NotebookBlock;
  content: Content;
  isActive: boolean;
  onSubmitGate: SubmitGate;
  onAnnotate?: Annotate;
}) {
  const [saved, patch] = usePersistedState(block.id, content.studentState, { chosen: null as string | null, failedChoices: [] as string[] }, onAnnotate);
  const chosen = asNullableString(saved.chosen);
  // Branches already tried and failed, in the order tried (kept across reloads).
  const failedChoices = asStringArray(saved.failedChoices);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const choose = async (choice: string) => {
    if (!isActive || busy || resolved || failedChoices.includes(choice)) return;
    patch({ chosen: choice });
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "branching_scenario_challenge", selectedChoice: choice });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
      if (!result.passed) patch({ failedChoices: [...failedChoices, choice] });
    } finally {
      setBusy(false);
    }
  };

  const chosenBranch = content.branches.find((b) => b.choice === chosen);

  if (resolved) {
    return (
      <div className="notebook-block-body">
        <RichText className="block-question" text={content.scenario} />
        {content.visualAid && <StaticVisual visual={content.visualAid} />}
        <p className="hint">{block.status === "passed" ? "Ya resolviste esta decisión." : "Esta decisión no se resolvió todavía: continuaremos con otra estrategia."}</p>
        {block.status === "passed" && chosenBranch && (
          <p className="hint">
            <InlineText text={chosenBranch.consequence} />
          </p>
        )}
      </div>
    );
  }

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);

  return (
    <div className="notebook-block-body">
      <RichText className="block-question" text={content.scenario} />
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <p className="hint">
        <InlineText text={content.decisionPoint} />
      </p>
      <div className="prediction-options">
        {content.branches.map((b) => {
          const discarded = failedChoices.includes(b.choice);
          return (
            <button
              key={b.choice}
              className={`${chosen === b.choice && !discarded ? "btn-primary" : ""}${discarded ? " option-discarded" : ""}`.trim()}
              onClick={() => void choose(b.choice)}
              disabled={!isActive || busy || discarded}
              aria-label={discarded ? `Opción descartada: ${b.choice}` : undefined}
            >
              <InlineText text={b.choice} />
            </button>
          );
        })}
      </div>
      {failedChoices.length > 0 && (
        <ul className="discarded-list">
          {failedChoices.map((choice) => {
            const branch = content.branches.find((b) => b.choice === choice);
            return (
              <li key={choice}>
                <span className="discarded-option">
                  <InlineText text={choice} />
                </span>
                {branch && (
                  <>
                    {" — "}
                    <InlineText text={branch.consequence} />
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {busy && <p className="hint">Calificando…</p>}
      {!busy && outcome && !outcome.passed && (
        <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "Esa no era la mejor opción — inténtalo de nuevo."}</p>
      )}
    </div>
  );
}
