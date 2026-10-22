import { useRef, useState } from "react";
import { DynamicSectionBlockSchema } from "../../../lib/schemas";
import type { DynamicSectionBlock, GateResult, GateSubmission, NotebookBlock } from "../../../lib/schemas";
import { AnchoredMicroTheoryBlock } from "./AnchoredMicroTheoryBlock";
import { BranchingScenarioChallengeBlock } from "./BranchingScenarioChallengeBlock";
import { DeclarativeVisualDiagramBlock } from "./DeclarativeVisualDiagramBlock";
import { HandsOnMissionBlock } from "./HandsOnMissionBlock";
import { HeuristicErrorAuditBlock } from "./HeuristicErrorAuditBlock";
import { InteractivePredictionGateBlock } from "./InteractivePredictionGateBlock";
import { MetacognitiveClosureBlock } from "./MetacognitiveClosureBlock";
import { SpacedInterleavedRetrievalBlock } from "./SpacedInterleavedRetrievalBlock";
import { DevBlockInfo } from "../../dev/DevBlockInfo";
import { BlockLexicalAssistant, TheorySelectionAssistant } from "../LexicalAssistantPopover";
import { BlockNote } from "./persist";
import type { Annotate } from "./persist";

const BLOCK_LABEL: Record<string, string> = {
  spaced_interleaved_retrieval: "Repaso rápido",
  anchored_micro_theory: "Teoría",
  declarative_visual_diagram: "Diagrama",
  branching_scenario_challenge: "Decisión",
  heuristic_error_audit: "Encuentra el error",
  interactive_prediction_gate: "Predice",
  hands_on_mission: "Misión",
  metacognitive_closure: "Cierre",
};

const STATUS_LABEL: Record<string, string> = {
  passed: "Superada",
  escalated: "Bloqueada — nuevo enfoque abajo",
  failed: "Inténtalo de nuevo",
};

export type SubmitGate = (blockId: string, submission: GateSubmission) => Promise<GateResult>;
/** Voluntary de-escalation: lock the active gate and get a more guided block. */
export type RequestEasierStep = (blockId: string, hintsShown: number, draft: string) => Promise<GateResult>;

const TERM_MAX_CHARS = 100;

/** Block types that ask the student a question — the ones that get private notes. */
const isQuestion = (t: string) =>
  t === "interactive_prediction_gate" || t === "branching_scenario_challenge" || t === "heuristic_error_audit" || t === "hands_on_mission";

/** What the popover assistant needs to answer well FOR THIS SPECIFIC BLOCK —
 *  never the whole lesson's content, just this block's own. `answerBearing`
 *  is a defensive redaction guard only (see `LexicalAssistantService::ask`),
 *  never sent into the assistant's own prompt. */
function deriveBlockLexicalContext(content: DynamicSectionBlock): { term: string; fragmentContext: string; answerBearing: string[] } {
  switch (content.blockType) {
    case "spaced_interleaved_retrieval":
      return {
        term: content.items[0]?.conceptLabel ?? "",
        fragmentContext: content.items.map((i) => i.prompt).join(" "),
        answerBearing: content.items.map((i) => i.expectedAnswer ?? ""),
      };
    case "anchored_micro_theory": {
      // All 4 layers (hook/boundary/rule/error) — `analogyBoundary` is
      // optional (absent on blocks persisted before it existed), so it's
      // filtered out rather than joined as an empty/undefined chunk. This
      // is also what the text-selection popover uses as `fragmentContext`
      // for a highlighted fragment anywhere in the block (see
      // `TheorySelectionAssistant` below), not just the hook/rule.
      const layers = [content.intuitiveHook, content.analogyBoundary, content.systemRule, content.frequentError].filter(
        (layer): layer is string => Boolean(layer && layer.trim()),
      );
      return { term: content.title, fragmentContext: layers.join(" "), answerBearing: [] };
    }
    case "declarative_visual_diagram":
      return { term: content.title, fragmentContext: content.title, answerBearing: [] };
    case "branching_scenario_challenge":
      return { term: content.scenario, fragmentContext: content.decisionPoint, answerBearing: content.branches.map((b) => b.choice) };
    case "heuristic_error_audit":
      return {
        term: content.instruction,
        fragmentContext: content.flawedRepresentation.context,
        answerBearing: content.modelSolution ? [content.modelSolution] : [],
      };
    case "interactive_prediction_gate":
      return {
        term: content.question,
        fragmentContext: [...content.options, ...(content.shuffledSteps ?? []), ...(content.boundaryCases ?? []).map((c) => c.statement)].join(", "),
        answerBearing: [...content.options, ...(content.shuffledSteps ?? [])],
      };
    case "hands_on_mission":
      return { term: content.challengeStatement, fragmentContext: content.expectedMilestoneArtifact, answerBearing: [] };
    case "metacognitive_closure":
      return { term: content.synthesisTask, fragmentContext: content.predictionComparison?.contrastNarrative ?? "", answerBearing: [] };
  }
}

/** The dynamic component registry: reads `block.content_json`, validates it
 *  against the pedagogical block catalog, and dispatches to the matching
 *  renderer — no fixed section grouping, just an ordered sequence chosen by
 *  `notebook_agent` per class, revealed one gate at a time (see
 *  `ClassNotebookView`). A block that fails to parse (unexpected shape)
 *  degrades to a small notice instead of crashing the whole notebook. */
export function DynamicNotebookBlock({
  block,
  isActive,
  lessonKeyConcepts,
  onSubmitGate,
  onRequestEasierStep,
  onAnnotate,
  onSave,
  onRequestClosureFeedback,
  onContinueModule,
  continueLabel,
  onRegenerate,
  regenerating,
  regenerateError,
}: {
  block: NotebookBlock;
  isActive: boolean;
  lessonKeyConcepts: string[];
  onSubmitGate: SubmitGate;
  onRequestEasierStep?: RequestEasierStep;
  /** Persists the student's notes and selection state on question blocks. */
  onAnnotate?: Annotate;
  onSave: (blockId: string, content: Record<string, unknown>) => void;
  onRequestClosureFeedback: (blockId: string, reflection: string) => Promise<{ passed: boolean; feedback: string }>;
  onContinueModule: () => void;
  continueLabel: string;
  /** Repairs a block whose stored content doesn't parse anymore: the backend
   *  regenerates it IN PLACE (same id, position and type) and the whole class
   *  is re-rendered from the returned payload. */
  onRegenerate?: (blockId: string) => void;
  regenerating?: boolean;
  regenerateError?: string | null;
}) {
  // One margin-comment slot per block (see `TheorySelectionAssistant`): the
  // ¿Dudas? trigger lives in the kicker row, the comment itself is owned by
  // the zone wrapper below — siblings, so their shared open state and the
  // element refs (block rect → margin position, trigger rect → fallback +
  // outside-click exemption) are hoisted HERE, before the parse early-return
  // so the hook order never depends on parse success (a regeneration can
  // flip it on the same mounted instance).
  const blockRef = useRef<HTMLElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const [assistantOpen, setAssistantOpen] = useState(false);
  const parsed = DynamicSectionBlockSchema.safeParse(block.content_json);
  if (!parsed.success) {
    return (
      <section className="notebook-block-wrapper unsupported">
        <p className="hint">Este bloque no se pudo mostrar (formato inesperado).</p>
        {regenerating ? (
          <p className="hint">Regenerando este bloque…</p>
        ) : (
          <div className="row">
            <button className="btn-quiet btn-retry" onClick={() => onRegenerate?.(block.id)} disabled={!onRegenerate}>
              Regenerar este bloque
            </button>
          </div>
        )}
        {regenerateError && <p className="hint hint-warn">{regenerateError}</p>}
        <DevBlockInfo block={block} issues={parsed.error.issues.map((i) => `${i.path.join(".") || "(raíz)"}: ${i.message}`)} />
      </section>
    );
  }
  const content = parsed.data;
  const statusLabel = block.status !== "ready" ? STATUS_LABEL[block.status] : undefined;
  const lexical = deriveBlockLexicalContext(content);

  return (
    <section className={`notebook-block-wrapper ${content.blockType} status-${block.status}`} ref={blockRef}>
      <div className="block-kicker">
        <span className="block-kicker-label">
          {BLOCK_LABEL[content.blockType] ?? content.blockType}
          {statusLabel && <span className={`badge ${block.status === "passed" ? "ok" : "warn"}`}>{statusLabel}</span>}
        </span>
        <BlockLexicalAssistant
          open={assistantOpen}
          onToggle={() => setAssistantOpen((o) => !o)}
          triggerRef={triggerRef}
        />
      </div>
      <DevBlockInfo block={block} />
      <TheorySelectionAssistant
        blockRef={blockRef}
        triggerRef={triggerRef}
        buttonOpen={assistantOpen}
        onButtonOpenChange={setAssistantOpen}
        buttonTerm={lexical.term.slice(0, TERM_MAX_CHARS)}
        fragmentContext={lexical.fragmentContext || null}
        keyConcepts={lessonKeyConcepts}
        answerBearingStrings={lexical.answerBearing}
      >
        {content.blockType === "spaced_interleaved_retrieval" && <SpacedInterleavedRetrievalBlock blockId={block.id} content={content} />}
        {content.blockType === "anchored_micro_theory" && <AnchoredMicroTheoryBlock content={content} />}
        {content.blockType === "declarative_visual_diagram" && <DeclarativeVisualDiagramBlock content={content} />}
        {content.blockType === "branching_scenario_challenge" && (
          <BranchingScenarioChallengeBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} onAnnotate={onAnnotate} />
        )}
        {content.blockType === "heuristic_error_audit" && (
          <HeuristicErrorAuditBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} onAnnotate={onAnnotate} />
        )}
        {content.blockType === "interactive_prediction_gate" && (
          <InteractivePredictionGateBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} onAnnotate={onAnnotate} />
        )}
        {content.blockType === "hands_on_mission" && (
          <HandsOnMissionBlock
            block={block}
            content={content}
            isActive={isActive}
            onSubmitGate={onSubmitGate}
            onRequestEasierStep={onRequestEasierStep}
            onAnnotate={onAnnotate}
          />
        )}
        {isQuestion(content.blockType) && (
          <BlockNote blockId={block.id} saved={"studentNote" in content ? content.studentNote : null} onAnnotate={onAnnotate} />
        )}
        {content.blockType === "metacognitive_closure" && (
          <MetacognitiveClosureBlock
            block={block}
            content={content}
            isActive={isActive}
            onSave={onSave}
            onRequestFeedback={onRequestClosureFeedback}
            onContinue={onContinueModule}
            continueLabel={continueLabel}
          />
        )}
      </TheorySelectionAssistant>
    </section>
  );
}
