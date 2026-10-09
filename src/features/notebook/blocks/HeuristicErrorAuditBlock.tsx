import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "heuristic_error_audit" }>;

/** A deliberately flawed artifact the student audits — writes their own
 *  root-cause diagnosis (free text), graded server-side by
 *  `notebook_gate_grader` against `modelSolution` (redacted here until the
 *  gate is actually passed). Wrong diagnoses get a scaffold hint, never the
 *  solution. */
export function HeuristicErrorAuditBlock({
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
  const [diagnosis, setDiagnosis] = useState("");
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const submit = async () => {
    if (!isActive || busy || resolved || !diagnosis.trim()) return;
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "heuristic_error_audit", diagnosisText: diagnosis.trim() });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
      if (result.passed) setDiagnosis("");
    } finally {
      setBusy(false);
    }
  };

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);

  return (
    <div className="notebook-block-body">
      <RichText text={content.instruction} />
      <p className="hint">
        <InlineText text={content.flawedRepresentation.context} />
      </p>
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <pre className="flawed-snippet">{content.flawedRepresentation.buggySnippetOrDiagram}</pre>
      <ul className="dod-list">
        {content.guidingQuestions.map((q) => (
          <li key={q}>
            <InlineText text={q} />
          </li>
        ))}
      </ul>
      {resolved ? (
        <>
          <p className="hint">{block.status === "passed" ? "Ya identificaste la causa raíz." : "Aún no se identificó la causa raíz: continuaremos con otra estrategia."}</p>
          {content.modelSolution && <RichText className="block-question" text={content.modelSolution} />}
        </>
      ) : (
        <>
          <textarea
            value={diagnosis}
            onChange={(e) => setDiagnosis(e.target.value)}
            placeholder="¿Cuál crees que es la causa raíz del error?"
            rows={3}
            disabled={!isActive || busy}
          />
          <div className="row">
            <button className="btn-primary" onClick={() => void submit()} disabled={!isActive || busy || !diagnosis.trim()}>
              {busy ? "Calificando…" : "Enviar diagnóstico"}
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
