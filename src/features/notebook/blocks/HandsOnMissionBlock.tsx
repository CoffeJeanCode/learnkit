import { useMemo, useRef, useState } from "react";
import type { DynamicSectionBlock, NotebookBlock } from "../../../lib/schemas";
import type { RequestEasierStep, SubmitGate } from "./index";
import { GateProgress, GateSubmitButton } from "./GateSubmitButton";
import { usePresentation } from "../../../stores/study";
import { isClick, nearRect, tokenMatches } from "./interaction";
import { asNumber, asNullableString, usePersistedState } from "./persist";
import type { Annotate } from "./persist";
import { InlineText, RichText } from "./RichText";
import { StaticVisual } from "./StaticVisual";
import { formatTechnicalProse, parseMilestoneDeliverable, unpackConstraints } from "./technicalFormat";
import {
  IconAlert,
  IconLightbulb,
  IconPackage,
  IconPencil,
  IconRestore,
  IconTarget,
  IconText,
  IconTrash,
} from "../../../components/Icons";

type Content = Extract<DynamicSectionBlock, { blockType: "hands_on_mission" }>;
type Step = NonNullable<Content["interactiveSteps"]>[number];

const REVEAL_AFTER_MISSES = 2;

const SUPPORT_LABEL: Record<string, string> = {
  full: "Apoyo total",
  guided: "Guiada",
  faded: "Con menos apoyo",
};

/**
 * Breaks dense run-on templates into clean rows and isolates rules/notes,
 * so the student can read each step with visual harmony instead of a wall of text.
 */
export function formatWorkedTemplate(raw: string): string {
  if (!raw.includes("\n")) {
    return raw
      .replace(/:\s*(?=(?:Fila|Paso|Item|Row|Step)\s*1\b)/gi, ":\n\n")
      .replace(/;\s*(?=(?:Fila|Paso|Item|Bloque|Línea|Line|Row|Step)\s*\d+|[A-Z][a-z]+:|\d+\.|const|let|var|return|\/\/)/gi, ";\n\n")
      .replace(/\.\s*(?=(?:Regla\s*que|Regla\s*:|Nota\s*:|Importante\s*:|Advertencia\s*:|Tip\s*:|Criterio\s*:))/gi, ".\n\n📌 ");
  }
  return raw;
}

function parseTemplateSlots(template: string): { parts: string[]; slotCount: number } {
  const parts = template.split("___");
  return {
    parts,
    slotCount: parts.length - 1,
  };
}

function assembleSubmission(parts: string[], values: string[]): string {
  let result = "";
  for (let i = 0; i < parts.length; i++) {
    result += parts[i];
    if (i < values.length) {
      result += values[i];
    }
  }
  return result;
}

/** One cloze micro-step: type a short token or pick a chip. */
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
  const socket = useRef<HTMLSpanElement>(null);
  const press = useRef<{ opt: string; x0: number; y0: number } | null>(null);
  const dragRef = useRef<{ opt: string; x: number; y: number; over: boolean; moved: boolean } | null>(null);
  const [drag, setDragState] = useState<{ opt: string; x: number; y: number; over: boolean; moved: boolean } | null>(null);
  const setDrag = (next: typeof drag) => {
    dragRef.current = next;
    setDragState(next);
  };

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
              <div className="cloze-bench">
                <span ref={socket} className={`cloze-socket${drag?.over ? " is-over" : ""}`} aria-hidden="true">
                  {drag?.over ? drag.opt : "___"}
                </span>
                <div className="cloze-chips" role="group" aria-label="Opciones: arrastra una al hueco o actívala">
                  {step.options.map((opt) => (
                    <button
                      key={opt}
                      className={`chip cloze-op${drag?.opt === opt && drag.moved ? " is-held" : ""}`}
                      disabled={discarded.includes(opt)}
                      onClick={(e) => e.detail === 0 && attempt(opt)}
                      onPointerDown={(e) => {
                        if (e.button !== 0) return;
                        e.currentTarget.setPointerCapture(e.pointerId);
                        press.current = { opt, x0: e.clientX, y0: e.clientY };
                        setDrag({ opt, x: e.clientX, y: e.clientY, over: false, moved: false });
                      }}
                      onPointerMove={(e) => {
                        const p = press.current;
                        if (!p) return;
                        const moved = !isClick(e.clientX - p.x0, e.clientY - p.y0);
                        const box = socket.current?.getBoundingClientRect();
                        setDrag({ opt: p.opt, x: e.clientX, y: e.clientY, moved, over: moved && !!box && nearRect(box, e.clientX, e.clientY, 28) });
                      }}
                      onPointerUp={(e) => {
                        const p = press.current;
                        const d = dragRef.current;
                        press.current = null;
                        setDrag(null);
                        if (!p) return;
                        if (isClick(e.clientX - p.x0, e.clientY - p.y0) || d?.over) attempt(p.opt);
                      }}
                      onPointerCancel={() => {
                        press.current = null;
                        setDrag(null);
                      }}
                    >
                      {opt}
                    </button>
                  ))}
                </div>
                {drag?.moved && (
                  <span className="cloze-ghost" style={{ left: drag.x, top: drag.y }} aria-hidden="true">
                    {drag.opt}
                  </span>
                )}
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
  const rawTemplate = content.workedTemplate?.trim() ? content.workedTemplate : null;
  const template = useMemo(() => (rawTemplate ? formatWorkedTemplate(rawTemplate) : null), [rawTemplate]);
  const steps = content.interactiveSteps ?? [];
  const hints = content.scaffoldingHints ?? [];

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
  const [hintsExpanded, setHintsExpanded] = useState(false);

  // Detección de huecos de plantilla
  const { parts, slotCount } = useMemo(() => {
    return template ? parseTemplateSlots(template) : { parts: [], slotCount: 0 };
  }, [template]);

  const hasSlots = slotCount > 0;
  const [editorMode, setEditorMode] = useState<"cloze" | "free">(hasSlots ? "cloze" : "free");
  const [slotValues, setSlotValues] = useState<string[]>(Array(slotCount).fill(""));
  const filledSlots = useMemo(
    () => slotValues.filter((v) => v.trim().length > 0).length,
    [slotValues],
  );

  const resolved = block.status === "passed" || block.status === "escalated";
  const stepsDone = solvedSteps >= steps.length;

  const formattedDeliverable = useMemo(
    () => parseMilestoneDeliverable(content.expectedMilestoneArtifact),
    [content.expectedMilestoneArtifact],
  );

  const formattedConstraints = useMemo(
    () => unpackConstraints(content.constraints),
    [content.constraints],
  );

  const formattedChallenge = useMemo(() => {
    let s = content.challengeStatement || "";
    if (/^El reto\s+(?=[A-Z¿¡])/i.test(s)) {
      s = s.replace(/^El reto\s+/i, "**El reto:** ");
    }
    return formatTechnicalProse(s);
  }, [content.challengeStatement]);

  // Blanks restantes en la submission
  const remainingBlanks = (submission.match(/___/g) || []).length;
  const unchangedTemplate = template !== null && submission.trim() === template.trim();
  const canSubmit = isActive && !busy && !easing && stepsDone && !!submission.trim() && !unchangedTemplate && remainingBlanks === 0;
  const disabledReason = !isActive
    ? "Este reto ya no está activo."
    : easing
      ? "Preparando un paso más guiado…"
      : !stepsDone
        ? "Completa los micro-pasos para habilitar el envío."
        : !submission.trim()
          ? "Escribe tu solución antes de enviarla."
          : unchangedTemplate
            ? "Modifica o completa la plantilla antes de enviar."
            : remainingBlanks > 0
              ? `Quedan ${remainingBlanks} hueco(s) sin rellenar en la plantilla.`
              : null;

  const handleSlotChange = (index: number, val: string) => {
    const next = [...slotValues];
    next[index] = val;
    setSlotValues(next);
    const assembled = assembleSubmission(parts, next);
    setSubmission(assembled);
  };

  const handleRestoreTemplate = () => {
    if (!template) return;
    setSubmission(template);
    setSlotValues(Array(slotCount).fill(""));
  };

  const handleClearAll = () => {
    setSubmission("");
    setSlotValues(Array(slotCount).fill(""));
    setEditorMode("free");
  };

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
    <div className="notebook-block-body space-y-4">
      {/* Guía visual de 3 pasos en lenguaje de cuaderno */}
      <div className="mission-stepper">
        <div className="mission-step-item">
          <span className="mission-step-num is-active">1</span>
          <span>Revisa el reto</span>
        </div>
        <span className="mission-step-arrow">→</span>
        <div className="mission-step-item">
          <span className={`mission-step-num ${stepsDone ? "is-active" : ""}`}>2</span>
          <span>Solución</span>
        </div>
        <span className="mission-step-arrow">→</span>
        <div className="mission-step-item">
          <span className={`mission-step-num ${resolved ? "is-done" : ""}`}>3</span>
          <span>Verificación</span>
        </div>
      </div>

      {content.isTransfer && showTransferFraming && (
        <p className="hint">
          <strong>Reto de transferencia:</strong> el mismo concepto en un caso nuevo, sin pistas.
        </p>
      )}
      {level && <span className="badge mission-level">{level}</span>}
      <RichText className="block-question text-base text-slate-100 font-medium leading-relaxed" text={formattedChallenge} />
      {content.visualAid && <StaticVisual visual={content.visualAid} />}

      {/* Tarjetas estructuradas de entregable y restricciones sin gradientes ni bordes artificiales */}
      <div className="mission-brief-grid">
        <div className="mission-brief-card deliverable">
          <div className="mission-brief-header">
            <span className="mission-brief-title">
              <IconPackage />
              <span>Entregable esperado</span>
            </span>
            <span className="mission-brief-badge">Meta a construir</span>
          </div>

          {formattedDeliverable.intro && (
            <div className="mission-brief-intro">
              <InlineText text={formattedDeliverable.intro} />
            </div>
          )}

          <ul className="mission-brief-list">
            {formattedDeliverable.items.map((item, idx) => (
              <li key={idx} className="mission-brief-item">
                <span className="mission-brief-item-num">{idx + 1}</span>
                <div className="flex-1 min-w-0">
                  <InlineText text={item.replace(/^\(\d+\)\s*/, "")} />
                </div>
              </li>
            ))}
          </ul>

          {formattedDeliverable.note && (
            <div className="mission-brief-note">
              <IconTarget />
              <div className="flex-1">
                <strong className="block text-[11px] uppercase tracking-wider mb-0.5">
                  Criterio de validación
                </strong>
                <InlineText text={formattedDeliverable.note} />
              </div>
            </div>
          )}
        </div>

        <div className="mission-brief-card constraints">
          <div className="mission-brief-header">
            <span className="mission-brief-title">
              <IconAlert />
              <span>Restricciones técnicas</span>
            </span>
            <span className="mission-brief-badge">Reglas clave</span>
          </div>

          <ul className="mission-brief-list">
            {formattedConstraints.map((constraint, idx) => (
              <li key={idx} className="mission-brief-item">
                <span className="mission-brief-item-bullet">✦</span>
                <div className="flex-1 min-w-0">
                  <InlineText text={formatTechnicalProse(constraint)} />
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>

      {resolved ? (
        <div className="mission-resolved-banner">
          {block.status === "passed" ? "✓ Reto superado exitosamente." : "Este reto no se superó todavía: continuaremos con un paso más guiado."}
        </div>
      ) : (
        <>
          {steps.length > 0 && (
            <div className="cloze-steps bg-slate-900/60 p-3 rounded-lg border border-slate-800">
              <div className="section-label text-xs font-semibold text-amber-300 mb-2">Micro-pasos de preparación:</div>
              <ol className="cloze-list space-y-2">
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

          {/* Cajón de Pistas y Ayuda */}
          {hints.length > 0 && (
            <div className="rounded-lg overflow-hidden bg-card/60">
              <button
                type="button"
                onClick={() => setHintsExpanded(!hintsExpanded)}
                className="w-full flex items-center justify-between px-3 py-2 text-xs font-medium text-slate-300 hover:bg-card transition-colors border-none cursor-pointer"
              >
                <span className="flex items-center gap-1.5">
                  <IconLightbulb className="text-amber-300" />
                  <span>Pistas y conceptos clave ({hintsShown}/{hints.length} desbloqueadas)</span>
                </span>
                <span>{hintsExpanded ? "▲ Ocultar" : "▼ Desplegar"}</span>
              </button>
              {hintsExpanded && (
                <div className="p-3 space-y-2 bg-paper-deep">
                  {hints.slice(0, hintsShown).map((h, i) => (
                    <div key={i} className="text-xs p-2 rounded bg-gold-wash text-gold-chalk">
                      <strong>Pista {i + 1}:</strong> <InlineText text={h} />
                    </div>
                  ))}
                  <button
                    type="button"
                    className="btn-quiet text-xs mt-1 inline-flex items-center gap-1.5"
                    onClick={() => patch({ hintsShown: Math.min(hints.length, hintsShown + 1) })}
                    disabled={!isActive || busy || easing || hintsShown >= hints.length}
                  >
                    <IconLightbulb />
                    <span>{hintsShown >= hints.length ? "Sin más pistas disponibles" : "Revelar siguiente pista"}</span>
                  </button>
                </div>
              )}
            </div>
          )}

          {stepsDone ? (
            <div className="space-y-2">
              {/* Barra de herramientas del editor */}
              <div className="flex flex-wrap items-center justify-between gap-2 pb-1">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-semibold text-slate-200">Tu Solución</span>
                  {hasSlots && (
                    <div className="editor-mode-toggle">
                      <button
                        type="button"
                        onClick={() => setEditorMode("cloze")}
                        className={`editor-mode-btn ${editorMode === "cloze" ? "is-active" : ""}`}
                      >
                        <IconPencil />
                        <span>Modo Huecos</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => setEditorMode("free")}
                        className={`editor-mode-btn ${editorMode === "free" ? "is-active" : ""}`}
                      >
                        <IconText />
                        <span>Editor Libre</span>
                      </button>
                    </div>
                  )}
                </div>

                <div className="flex items-center gap-2">
                  {template && (
                    <button
                      type="button"
                      className="editor-action-btn"
                      onClick={handleRestoreTemplate}
                      disabled={!isActive || busy || easing || submission === template}
                      title="Restaura la plantilla inicial con los huecos ___"
                    >
                      <IconRestore />
                      <span>Restaurar plantilla</span>
                    </button>
                  )}
                  <button
                    type="button"
                    className="editor-action-btn is-danger"
                    onClick={handleClearAll}
                    disabled={!isActive || busy || easing || !submission}
                    title="Borra todo el texto para escribir libremente desde cero"
                  >
                    <IconTrash />
                    <span>Borrar todo</span>
                  </button>
                </div>
              </div>

              {/* Modo Huecos Interactivos en formato cuaderno */}
              {hasSlots && editorMode === "cloze" ? (
                <div className="cloze-code-shell">
                  <div className="cloze-code-shell-header">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-slate-200">Completa los huecos de la plantilla:</span>
                      <span
                        className="mission-brief-badge"
                        style={{
                          background: filledSlots === slotCount ? "var(--status-success-surface)" : "var(--status-warn-surface)",
                          color: filledSlots === slotCount ? "var(--status-success-text)" : "var(--status-warn-text)",
                        }}
                      >
                        {filledSlots} de {slotCount} listos
                      </span>
                    </div>
                    <span className="text-xs" style={{ color: "var(--text-soft)" }}>
                      {filledSlots === slotCount ? "✓ Listo para enviar" : `Faltan ${slotCount - filledSlots} por completar`}
                    </span>
                  </div>
                  <pre className="cloze-pre select-text">
                    {parts.map((part, i) => (
                      <span key={i}>
                        {part}
                        {i < slotCount && (
                          <span className="inline-flex items-center align-middle mx-1 my-0.5">
                            <input
                              type="text"
                              value={slotValues[i] || ""}
                              onChange={(e) => handleSlotChange(i, e.target.value)}
                              placeholder={`(${i + 1})`}
                              disabled={!isActive || busy || easing}
                              style={{
                                width: `${Math.max(5, (slotValues[i] || "").length + 2)}ch`,
                              }}
                              className={`cloze-slot ${slotValues[i]?.trim() ? "is-filled" : ""}`}
                              aria-label={`Hueco ${i + 1} de ${slotCount}`}
                            />
                          </span>
                        )}
                      </span>
                    ))}
                  </pre>
                </div>
              ) : (
                /* Modo Editor Libre */
                <textarea
                  value={submission}
                  onChange={(e) => setSubmission(e.target.value)}
                  placeholder="Escribe, modifica o pega tu solución aquí libremente..."
                  rows={template ? 10 : 6}
                  disabled={!isActive || busy || easing}
                  className="mission-editor w-full p-4 font-mono text-xs rounded-xl bg-input text-slate-200 leading-relaxed border-none focus:outline-none"
                />
              )}
            </div>
          ) : (
            <p className="text-xs text-slate-400 italic bg-paper-deep p-3 rounded-lg">
              Resuelve primero los micro-pasos preparatorios para habilitar el área de solución.
            </p>
          )}

          {/* Botones de acción principales */}
          <div className="flex flex-wrap items-center gap-2 pt-2">
            <GateSubmitButton
              label="Enviar mi solución"
              busy={busy}
              disabled={!canSubmit}
              disabledReason={disabledReason}
              onClick={() => void submit()}
            />

            {onRequestEasierStep && (
              <button
                className="btn-quiet text-xs"
                onClick={() => void askForEasierStep()}
                disabled={!isActive || busy || easing}
              >
                {easing ? "Preparando paso más guiado…" : "¿Muy difícil? Pedir paso más guiado"}
              </button>
            )}
          </div>
          <GateProgress busy={busy} label="Calificando la solución" />

          {error && <p className="hint hint-warn text-rose-400 text-xs">{error}</p>}
          {!busy && outcome && !outcome.passed && (
            <div className="mission-feedback-banner">
              <strong className="flex items-center gap-1.5 text-xs">
                <IconAlert className="w-3.5 h-3.5" />
                <span>Orientación pedagógica:</span>
              </strong>
              <div>{shownFeedback ? <InlineText text={shownFeedback} /> : "Tu solución aún no cumple todos los criterios. Revisa los huecos e inténtalo de nuevo."}</div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
