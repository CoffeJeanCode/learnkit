import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { answerKey, moveItem } from "./interaction";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "interactive_prediction_gate" }>;
type Outcome = { passed: boolean; feedback: string | null };

/** A hypothesis question that GATES the next block — the student commits to
 *  an answer, which is graded server-side (the answer keys never reach this
 *  component). Three modes: multiple choice (default), ordering the links of a
 *  causal chain, and classifying boundary cases. Wrong answers stay locked on
 *  this same block (up to 3 attempts) with a hint, never the answer; the 3rd
 *  wrong answer escalates to a re-approach block instead. An answer already
 *  tried and failed can't be submitted again, so the gate can't be solved by
 *  clicking through the possibilities. */
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
  const mode = content.interactionMode ?? "multiple_choice";
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  // Multiple choice: the options already tried and failed, in the order tried.
  const [failedOptions, setFailedOptions] = useState<string[]>([]);
  const [lastOption, setLastOption] = useState<string | null>(null);
  // Causal chain / boundary cases: answers already tried and failed.
  const [failedKeys, setFailedKeys] = useState<string[]>([]);
  const [order, setOrder] = useState<string[]>(content.shuffledSteps ?? []);
  const cases = content.boundaryCases ?? [];
  const [verdicts, setVerdicts] = useState<(boolean | null)[]>(() => cases.map(() => null));
  const resolved = block.status === "passed" || block.status === "escalated";

  const send = async (submission: Parameters<SubmitGate>[1], onWrong: () => void) => {
    if (!isActive || busy || resolved) return;
    setBusy(true);
    setOutcome(null);
    try {
      const result = await onSubmitGate(block.id, submission);
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
      if (!result.passed) onWrong();
    } finally {
      setBusy(false);
    }
  };

  const choose = (option: string) => {
    setLastOption(option);
    void send({ blockType: "interactive_prediction_gate", selectedOption: option }, () => setFailedOptions((f) => (f.includes(option) ? f : [...f, option])));
  };

  const orderKey = answerKey(order);
  const checkOrder = () =>
    void send({ blockType: "interactive_prediction_gate", orderedSteps: order }, () => setFailedKeys((f) => [...f, orderKey]));

  const allClassified = verdicts.every((v) => v !== null);
  const verdictKey = answerKey(verdicts);
  const checkCases = () =>
    void send({ blockType: "interactive_prediction_gate", caseVerdicts: verdicts.map((v) => v === true) }, () => setFailedKeys((f) => [...f, verdictKey]));

  const header = (
    <>
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <RichText className="block-question" text={content.question} />
    </>
  );

  if (resolved) {
    const explanation = block.status === "passed" && lastOption ? content.conceptualFeedbackMap[lastOption] : null;
    return (
      <div className="notebook-block-body">
        {header}
        <p className="hint">{block.status === "passed" ? "Ya superaste esta predicción." : "Esta predicción no se superó todavía: continuaremos con otra estrategia."}</p>
        {explanation && (
          <p className="hint">
            <InlineText text={explanation} />
          </p>
        )}
      </div>
    );
  }

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);
  const warning = !busy && outcome && !outcome.passed && (
    <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "No es correcto — inténtalo de nuevo."}</p>
  );

  if (mode === "causal_chain_order") {
    const alreadyTried = failedKeys.includes(orderKey);
    return (
      <div className="notebook-block-body">
        {header}
        <p className="hint">Ordena los eslabones de la cadena: primero arriba, último abajo.</p>
        <ol className="chain-list">
          {order.map((step, i) => (
            <li key={step} className="chain-item">
              <span className="chain-no">{i + 1}</span>
              <span className="chain-text">
                <InlineText text={step} />
              </span>
              <span className="chain-moves">
                <button className="btn-quiet" onClick={() => setOrder((o) => moveItem(o, i, -1))} disabled={!isActive || busy || i === 0} aria-label={`Subir el paso ${i + 1}`}>
                  ↑
                </button>
                <button
                  className="btn-quiet"
                  onClick={() => setOrder((o) => moveItem(o, i, 1))}
                  disabled={!isActive || busy || i === order.length - 1}
                  aria-label={`Bajar el paso ${i + 1}`}
                >
                  ↓
                </button>
              </span>
            </li>
          ))}
        </ol>
        <div className="row">
          <button className="btn-primary" onClick={checkOrder} disabled={!isActive || busy || alreadyTried}>
            {busy ? "Calificando…" : "Comprobar secuencia"}
          </button>
          {alreadyTried && <span className="hint">Ya probaste este orden — cámbialo antes de volver a comprobar.</span>}
        </div>
        {busy && <p className="hint">Calificando…</p>}
        {warning}
      </div>
    );
  }

  if (mode === "boundary_case_audit") {
    const alreadyTried = failedKeys.includes(verdictKey);
    return (
      <div className="notebook-block-body">
        {header}
        <p className="hint">Clasifica cada caso: ¿cumple la regla o la rompe?</p>
        <ul className="case-list">
          {cases.map((c, i) => (
            <li key={i} className="case-item">
              <p className="case-statement">
                <InlineText text={c.statement} />
              </p>
              <div className="case-choices" role="group" aria-label={`Caso ${i + 1}`}>
                {([true, false] as const).map((value) => (
                  <button
                    key={String(value)}
                    className={verdicts[i] === value ? "btn-primary" : ""}
                    aria-pressed={verdicts[i] === value}
                    onClick={() => setVerdicts((v) => v.map((x, j) => (j === i ? value : x)))}
                    disabled={!isActive || busy}
                  >
                    {value ? "Cumple la regla" : "Rompe la regla"}
                  </button>
                ))}
              </div>
            </li>
          ))}
        </ul>
        <div className="row">
          <button className="btn-primary" onClick={checkCases} disabled={!isActive || busy || !allClassified || alreadyTried}>
            {busy ? "Calificando…" : "Comprobar casos"}
          </button>
          {!allClassified && isActive && <span className="hint">Clasifica todos los casos para comprobar.</span>}
          {alreadyTried && <span className="hint">Ya probaste esta clasificación — cambia algún caso.</span>}
        </div>
        {busy && <p className="hint">Calificando…</p>}
        {warning}
      </div>
    );
  }

  return (
    <div className="notebook-block-body">
      {header}
      <div className="prediction-options">
        {content.options.map((opt) => {
          const discarded = failedOptions.includes(opt);
          return (
            <button
              key={opt}
              className={`${lastOption === opt && !discarded ? "btn-primary" : ""}${discarded ? " option-discarded" : ""}`.trim()}
              onClick={() => choose(opt)}
              disabled={!isActive || busy || discarded}
              aria-label={discarded ? `Opción descartada: ${opt}` : undefined}
            >
              <InlineText text={opt} />
            </button>
          );
        })}
      </div>
      {failedOptions.length > 0 && (
        <ul className="discarded-list">
          {failedOptions.map((opt) => (
            <li key={opt}>
              <span className="discarded-option">
                <InlineText text={opt} />
              </span>
              {content.conceptualFeedbackMap[opt] && (
                <>
                  {" — "}
                  <InlineText text={content.conceptualFeedbackMap[opt]} />
                </>
              )}
            </li>
          ))}
        </ul>
      )}
      {busy && <p className="hint">Calificando…</p>}
      {warning}
    </div>
  );
}
