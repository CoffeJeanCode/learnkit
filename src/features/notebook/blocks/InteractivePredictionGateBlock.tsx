import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { SubmitGate } from "./index";
import { answerKey } from "./interaction";
import { CaseWiring, ChainOrder } from "./nodes";
import { asNullableString, asStringArray, usePersistedState } from "./persist";
import type { Annotate } from "./persist";
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
  onAnnotate,
}: {
  block: NotebookBlock;
  content: Content;
  isActive: boolean;
  onSubmitGate: SubmitGate;
  onAnnotate?: Annotate;
}) {
  const mode = content.interactionMode ?? "multiple_choice";
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const shuffled = content.shuffledSteps ?? [];
  const cases = content.boundaryCases ?? [];
  // Everything the student has selected survives a reload: wrong picks stay
  // discarded, the arranged order and the case verdicts are restored.
  const [saved, patch] = usePersistedState(
    block.id,
    content.studentState,
    { failedOptions: [] as string[], lastOption: null as string | null, failedKeys: [] as string[], order: shuffled, verdicts: cases.map(() => null as boolean | null) },
    onAnnotate,
  );
  // Multiple choice: the options already tried and failed, in the order tried.
  const failedOptions = asStringArray(saved.failedOptions);
  const lastOption = asNullableString(saved.lastOption);
  // Causal chain / boundary cases: answers already tried and failed.
  const failedKeys = asStringArray(saved.failedKeys);
  const storedOrder = asStringArray(saved.order);
  const order = storedOrder.length === shuffled.length && [...storedOrder].sort().join("\u0000") === [...shuffled].sort().join("\u0000") ? storedOrder : shuffled;
  const verdicts: (boolean | null)[] =
    Array.isArray(saved.verdicts) && saved.verdicts.length === cases.length
      ? saved.verdicts.map((v) => (typeof v === "boolean" ? v : null))
      : cases.map(() => null);
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
    patch({ lastOption: option });
    void send({ blockType: "interactive_prediction_gate", selectedOption: option }, () =>
      patch({ failedOptions: failedOptions.includes(option) ? failedOptions : [...failedOptions, option] }),
    );
  };

  const orderKey = answerKey(order);
  const checkOrder = () =>
    void send({ blockType: "interactive_prediction_gate", orderedSteps: order }, () => patch({ failedKeys: [...failedKeys, orderKey] }));

  const allClassified = verdicts.every((v) => v !== null);
  const verdictKey = answerKey(verdicts);
  const checkCases = () =>
    void send({ blockType: "interactive_prediction_gate", caseVerdicts: verdicts.map((v) => v === true) }, () => patch({ failedKeys: [...failedKeys, verdictKey] }));

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
        <p className="hint">Arrastra cada eslabón hasta su lugar: primero arriba, último abajo.</p>
        <ChainOrder steps={order} onChange={(next) => patch({ order: next })} disabled={!isActive || busy} render={(step) => <InlineText text={step} />} />
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
        <p className="hint">Conecta cada caso con lo que le corresponde: arrastra desde su punto hasta «Cumple» o «Rompe».</p>
        <CaseWiring
          cases={cases.map((c, i) => ({ key: `${i}:${c.statement}`, content: <InlineText text={c.statement} /> }))}
          verdicts={verdicts}
          onChange={(i, value) => patch({ verdicts: verdicts.map((x, j) => (j === i ? value : x)) })}
          disabled={!isActive || busy}
        />
        <div className="row">
          <button className="btn-primary" onClick={checkCases} disabled={!isActive || busy || !allClassified || alreadyTried}>
            {busy ? "Calificando…" : "Comprobar casos"}
          </button>
          {!allClassified && isActive && <span className="hint">Conecta todos los casos para comprobar.</span>}
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
