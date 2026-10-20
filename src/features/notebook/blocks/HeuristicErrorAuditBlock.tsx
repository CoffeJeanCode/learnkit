import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { splitSnippetLines } from "./interaction";
import { asNullableString, asNumber, usePersistedState } from "./persist";
import type { Annotate } from "./persist";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "heuristic_error_audit" }>;

/** The flawed artifact as numbered, clickable rows ("cacería de bugs"): the
 *  student points at the line where the error lives BEFORE writing why. Once
 *  the gate is passed the real faulty line (revealed by the backend together
 *  with `modelSolution`) is highlighted. */
function BugLines({
  lines,
  selected,
  revealed,
  disabled,
  onSelect,
}: {
  lines: string[];
  selected: number | null;
  revealed: number | null;
  disabled: boolean;
  onSelect: (line: number) => void;
}) {
  return (
    <ol className="bug-lines" aria-label="Líneas del código a auditar">
      {lines.map((text, i) => {
        const n = i + 1;
        const cls = ["bug-line", selected === n ? "is-selected" : "", revealed === n ? "is-faulty" : ""].filter(Boolean).join(" ");
        return (
          <li key={n}>
            <button className={cls} onClick={() => onSelect(n)} disabled={disabled} aria-pressed={selected === n} aria-label={`Línea ${n}`}>
              <span className="bug-line-no">{n}</span>
              <code className="bug-line-code">{text || " "}</code>
            </button>
          </li>
        );
      })}
    </ol>
  );
}

/** A deliberately flawed artifact the student audits: first they click the
 *  line where the error occurs, then write their own root-cause diagnosis
 *  (free text), graded server-side by `notebook_gate_grader` against
 *  `modelSolution` (redacted here until the gate is actually passed). Wrong
 *  diagnoses get a scaffold hint, never the solution. */
export function HeuristicErrorAuditBlock({
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
  // The flagged line and the diagnosis draft survive a reload.
  const [saved, patch] = usePersistedState(block.id, content.studentState, { selectedLine: 0, diagnosis: "" }, onAnnotate);
  const diagnosis = asNullableString(saved.diagnosis) ?? "";
  const storedLine = asNumber(saved.selectedLine);
  const selectedLine = storedLine > 0 ? storedLine : null;
  const setDiagnosis = (value: string) => patch({ diagnosis: value });
  const setSelectedLine = (line: number) => patch({ selectedLine: line });
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const lines = splitSnippetLines(content.flawedRepresentation.buggySnippetOrDiagram);
  const lineInRange = selectedLine !== null && selectedLine <= lines.length;
  // A one-liner has nothing to point at: it keeps the plain diagnosis flow.
  const pickable = lines.length > 1;
  const needsLine = pickable && !lineInRange;

  const submit = async () => {
    if (!isActive || busy || resolved || !diagnosis.trim() || needsLine) return;
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, {
        blockType: "heuristic_error_audit",
        diagnosisText: diagnosis.trim(),
        selectedLine,
      });
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
      {pickable ? (
        <>
          {!resolved && <div className="section-label">1 · Señala la línea donde ocurre el error</div>}
          <BugLines
            lines={lines}
            selected={selectedLine}
            revealed={block.status === "passed" ? (content.faultyLineNumber ?? null) : null}
            disabled={!isActive || busy || resolved}
            onSelect={setSelectedLine}
          />
        </>
      ) : (
        <pre className="flawed-snippet">{content.flawedRepresentation.buggySnippetOrDiagram}</pre>
      )}
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
          {pickable && <div className="section-label">2 · Explica la causa raíz</div>}
          <textarea
            value={diagnosis}
            onChange={(e) => setDiagnosis(e.target.value)}
            placeholder={selectedLine ? `¿Por qué la línea ${selectedLine} es la causa raíz del error?` : "¿Cuál crees que es la causa raíz del error?"}
            rows={3}
            disabled={!isActive || busy}
          />
          <div className="row">
            <button className="btn-primary" onClick={() => void submit()} disabled={!isActive || busy || !diagnosis.trim() || needsLine}>
              {busy ? "Calificando…" : "Enviar diagnóstico"}
            </button>
            {needsLine && isActive && <span className="hint">Primero señala una línea.</span>}
          </div>
          {!busy && outcome && !outcome.passed && (
            <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "Todavía no — inténtalo de nuevo."}</p>
          )}
        </>
      )}
    </div>
  );
}
