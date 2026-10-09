import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { InlineText, RichText } from "./RichText";
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
      {content.isTransfer && (
        <p className="hint">
          <strong>Reto de transferencia:</strong> el mismo concepto en un caso nuevo, sin pistas.
        </p>
      )}
      <RichText className="block-question" text={content.challengeStatement} />
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <p className="hint">
        Entregable: <InlineText text={content.expectedMilestoneArtifact} />
      </p>
      <p className="hint">
        Restricciones: <InlineText text={content.constraints.join(" · ")} />
      </p>
      {content.scaffoldingHints.length > 0 && (
        <ul className="dod-list">
          {content.scaffoldingHints.map((h) => (
            <li key={h}>
              <InlineText text={h} />
            </li>
          ))}
        </ul>
      )}
      <p className="hint">
        Te evaluarás en: <InlineText text={content.evaluationRubricSummary.join(" · ")} />
      </p>
      {resolved ? (
        <p className="hint">
          {block.status === "passed" ? "Reto superado." : "Este reto no se superó todavía: continuaremos con otra estrategia."}
        </p>
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
          {!busy && outcome && !outcome.passed && (
            <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "Todavía no — inténtalo de nuevo."}</p>
          )}
        </>
      )}
    </div>
  );
}
