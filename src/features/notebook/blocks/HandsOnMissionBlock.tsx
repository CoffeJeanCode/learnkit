import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "hands_on_mission" }>;

/** Deliberate practice with real constraints — the student submits their
 *  solution as free text, graded server-side by `notebook_gate_grader`
 *  against `evaluationRubricSummary`. Passing is purely server-asserted
 *  (`status === "passed"`); there is no self-reported "done" checkbox. */
export function HandsOnMissionBlock({
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
  const [submission, setSubmission] = useState("");
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const submit = async () => {
    if (!isActive || busy || resolved || !submission.trim()) return;
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "hands_on_mission", submissionText: submission.trim() });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
      if (result.passed) setSubmission("");
    } finally {
      setBusy(false);
    }
  };

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);

  return (
    <div className="notebook-block-body">
      <p className="block-question">{content.challengeStatement}</p>
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <p className="hint">Entregable: {content.expectedMilestoneArtifact}</p>
      <p className="hint">Restricciones: {content.constraints.join(" · ")}</p>
      <ul className="dod-list">
        {content.scaffoldingHints.map((h) => (
          <li key={h}>{h}</li>
        ))}
      </ul>
      <p className="hint">Te evaluarás en: {content.evaluationRubricSummary.join(" · ")}</p>
      {resolved ? (
        <p className="hint">Superaste este reto.</p>
      ) : (
        <>
          <textarea
            value={submission}
            onChange={(e) => setSubmission(e.target.value)}
            placeholder="Describe o pega tu solución…"
            rows={4}
            disabled={!isActive || busy}
          />
          <div className="row">
            <button className="btn-primary" onClick={() => void submit()} disabled={!isActive || busy || !submission.trim()}>
              {busy ? "Calificando…" : "Enviar mi solución"}
            </button>
          </div>
          {!busy && outcome && !outcome.passed && <p className="hint hint-warn">{shownFeedback ?? "Todavía no — inténtalo de nuevo."}</p>}
        </>
      )}
    </div>
  );
}
