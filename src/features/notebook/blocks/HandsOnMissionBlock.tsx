import { useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { RequestEasierStep, SubmitGate } from "./index";
import { usePresentation } from "../../../stores/study";
import { tokenMatches } from "./interaction";
import { asNumber, asNullableString, usePersistedState } from "./persist";
import type { Annotate } from "./persist";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";

type Content = Extract<DynamicSectionBlock, { blockType: "hands_on_mission" }>;
type Step = NonNullable<Content["interactiveSteps"]>[number];

/** Wrong checks on one cloze step before the answer can be revealed — a
 *  micro-step must never trap the student on an ambiguous hole. */
const REVEAL_AFTER_MISSES = 2;

const SUPPORT_LABEL: Record<string, string> = {
  full: "Apoyo total",
  guided: "Guiada",
  faded: "Con menos apoyo",
};

/** One cloze micro-step: type a short token or pick a chip. Formative only —
 *  checked in the browser, never graded as a gate. Wrong chips are discarded
 *  so the answer can't be found by clicking through them. */
function ClozeStep({
  step,
  index,
  locked,
  done,
  onSolved,
}: {
  step: Step;
  index: number;
  locked: boolean;
  done: boolean;
  onSolved: () => void;
}) {
  const [typed, setTyped] = useState("");
  const [misses, setMisses] = useState(0);
  const [discarded, setDiscarded] = useState<string[]>([]);

  const attempt = (value: string) => {
    if (tokenMatches(value, step.expectedToken)) {
      onSolved();
    } else {
      setMisses((n) => n + 1);
      if (step.options) setDiscarded((d) => [...d, value]);
    }
  };

  return (
    <li className={`cloze-step${done ? " is-done" : ""}${locked ? " is-locked" : ""}`}>
      <div className="cloze-prompt">
        <span className="cloze-number">{done ? "✓" : index + 1}</span>
        <InlineText text={step.prompt} />
      </div>
      {done ? (
        <p className="cloze-answer">
          <code className="inline-code">{step.expectedToken}</code>
        </p>
      ) : (
        !locked && (
          <>
            {step.options ? (
              <div className="cloze-chips">
                {step.options.map((opt) => (
                  <button key={opt} className="chip" disabled={discarded.includes(opt)} onClick={() => attempt(opt)}>
                    {opt}
                  </button>
                ))}
              </div>
            ) : (
              <div className="row">
                <input
                  className="cloze-input"
                  value={typed}
                  onChange={(e) => setTyped(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && typed.trim()) attempt(typed);
                  }}
                  placeholder="Completa el hueco…"
                  aria-label={`Respuesta del paso ${index + 1}`}
                />
                <button className="btn-quiet" disabled={!typed.trim()} onClick={() => attempt(typed)}>
                  Comprobar
                </button>
              </div>
            )}
            {misses > 0 && <p className="hint hint-warn">Todavía no — relee el paso e inténtalo de nuevo.</p>}
            {misses >= REVEAL_AFTER_MISSES && (
              <button className="btn-quiet" onClick={onSolved}>
                Mostrar la respuesta
              </button>
            )}
          </>
        )
      )}
    </li>
  );
}

/** Deliberate practice with real constraints, with the scaffolding a
 *  struggling student needs: a pre-filled worked template (never a blank
 *  page), cloze micro-steps before the open editor, hints opened one at a
 *  time, and a way to ask for an easier step without failing 3 times. The
 *  solution is graded server-side by `notebook_gate_grader` against
 *  `evaluationRubricSummary`; passing is purely server-asserted. */
export function HandsOnMissionBlock({
  block,
  content,
  isActive,
  onSubmitGate,
  onRequestEasierStep,
  onAnnotate,
}: {
  block: NotebookBlock;
  content: Content;
  isActive: boolean;
  onSubmitGate: SubmitGate;
  onRequestEasierStep?: RequestEasierStep;
  onAnnotate?: Annotate;
}) {
  const { showTransferFraming } = usePresentation();
  const template = content.workedTemplate?.trim() ? content.workedTemplate : null;
  const steps = content.interactiveSteps ?? [];
  const hints = content.scaffoldingHints;

  // Draft, solved micro-steps and opened hints survive a reload (an opened
  // hint stays opened, so `hintsShown` evidence can't be reset by reloading).
  const [saved, patch] = usePersistedState(
    block.id,
    content.studentState,
    { submission: template ?? "", solvedSteps: 0, hintsShown: 0 },
    onAnnotate,
  );
  const submission = asNullableString(saved.submission) ?? (template ?? "");
  const solvedSteps = Math.min(steps.length, asNumber(saved.solvedSteps));
  const hintsShown = Math.min(hints.length, asNumber(saved.hintsShown));
  const setSubmission = (value: string) => patch({ submission: value });
  const [busy, setBusy] = useState(false);
  const [easing, setEasing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<{ passed: boolean; feedback: string | null } | null>(null);
  const resolved = block.status === "passed" || block.status === "escalated";

  const stepsDone = solvedSteps >= steps.length;
  const unchangedTemplate = template !== null && submission.trim() === template.trim();
  const canSubmit = isActive && !busy && !easing && stepsDone && !!submission.trim() && !unchangedTemplate;

  const submit = async () => {
    if (!canSubmit || resolved) return;
    setBusy(true);
    setOutcome(null);
    setError(null);
    try {
      const result = await onSubmitGate(block.id, { blockType: "hands_on_mission", submissionText: submission.trim(), hintsShown });
      setOutcome({ passed: result.passed, feedback: result.feedback ?? null });
      if (result.passed) setSubmission("");
    } finally {
      setBusy(false);
    }
  };

  const askForEasierStep = async () => {
    if (!onRequestEasierStep || !isActive || busy || easing || resolved) return;
    setEasing(true);
    setOutcome(null);
    setError(null);
    try {
      await onRequestEasierStep(block.id, hintsShown, submission);
    } catch (e) {
      setError(e instanceof Error ? e.message : "No se pudo preparar un paso más guiado. Inténtalo de nuevo.");
    } finally {
      setEasing(false);
    }
  };

  const shownFeedback = outcome?.feedback ?? (block.status === "failed" ? block.last_feedback : null);
  const level = content.supportLevel && !content.isTransfer ? SUPPORT_LABEL[content.supportLevel] : undefined;

  return (
    <div className="notebook-block-body">
      {content.isTransfer && showTransferFraming && (
        <p className="hint">
          <strong>Reto de transferencia:</strong> el mismo concepto en un caso nuevo, sin pistas.
        </p>
      )}
      {level && <span className="badge mission-level">{level}</span>}
      <RichText className="block-question" text={content.challengeStatement} />
      {content.visualAid && <StaticVisual visual={content.visualAid} />}
      <p className="hint">
        Entregable: <InlineText text={content.expectedMilestoneArtifact} />
      </p>
      <p className="hint">
        Restricciones: <InlineText text={content.constraints.join(" · ")} />
      </p>
      <p className="hint">
        Te evaluarás en: <InlineText text={content.evaluationRubricSummary.join(" · ")} />
      </p>
      {resolved ? (
        <p className="hint">
          {block.status === "passed" ? "Reto superado." : "Este reto no se superó todavía: continuaremos con un paso más guiado."}
        </p>
      ) : (
        <>
          {steps.length > 0 && (
            <div className="cloze-steps">
              <div className="section-label">Antes del reto: completa los huecos</div>
              <ol className="cloze-list">
                {steps.map((step, i) => (
                  <ClozeStep
                    key={`${i}-${step.prompt}`}
                    step={step}
                    index={i}
                    done={i < solvedSteps}
                    locked={i > solvedSteps || !isActive}
                    onSolved={() => patch({ solvedSteps: Math.max(solvedSteps, i + 1) })}
                  />
                ))}
              </ol>
            </div>
          )}

          {hints.length > 0 && (
            <div className="hint-stack">
              {hints.slice(0, hintsShown).map((h, i) => (
                <p key={i} className="hint-revealed">
                  <span className="hint-revealed-no">💡 {i + 1}</span> <InlineText text={h} />
                </p>
              ))}
              <button
                className="btn-quiet"
                onClick={() => patch({ hintsShown: Math.min(hints.length, hintsShown + 1) })}
                disabled={!isActive || busy || easing || hintsShown >= hints.length}
              >
                💡 {hintsShown >= hints.length ? "Sin más pistas" : "Pedir pista"} ({hintsShown}/{hints.length})
              </button>
            </div>
          )}

          {stepsDone ? (
            <>
              {template && <p className="hint">Parte de esta plantilla y completa los huecos (<code className="inline-code">___</code>).</p>}
              <textarea
                value={submission}
                onChange={(e) => setSubmission(e.target.value)}
                placeholder="Describe o pega tu solución…"
                rows={template ? 8 : 4}
                disabled={!isActive || busy || easing}
                className={template ? "mission-editor mission-editor-code" : "mission-editor"}
              />
              {unchangedTemplate && <p className="hint">Completa los huecos de la plantilla para poder enviar.</p>}
            </>
          ) : (
            <p className="hint">Resuelve los micro-pasos para abrir el área de tu solución.</p>
          )}
          <div className="row">
            <button className="btn-primary" onClick={() => void submit()} disabled={!canSubmit}>
              {busy ? "Calificando…" : "Enviar mi solución"}
            </button>
            {template && stepsDone && (
              <button className="btn-quiet" onClick={() => setSubmission(template)} disabled={!isActive || busy || easing || submission === template}>
                Restaurar plantilla
              </button>
            )}
            {onRequestEasierStep && (
              <button className="btn-quiet" onClick={() => void askForEasierStep()} disabled={!isActive || busy || easing}>
                {easing ? "Preparando un paso más guiado…" : "Necesito un paso previo más guiado"}
              </button>
            )}
          </div>
          {error && <p className="hint hint-warn">{error}</p>}
          {!busy && outcome && !outcome.passed && (
            <p className="hint hint-warn">{shownFeedback ? <InlineText text={shownFeedback} /> : "Todavía no — inténtalo de nuevo."}</p>
          )}
        </>
      )}
    </div>
  );
}
