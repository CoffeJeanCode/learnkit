import { DynamicSectionBlockSchema } from "../../../lib/schemas";
import type { DynamicSectionBlock, GateResult, GateSubmission, NotebookBlock } from "../../../lib/schemas";
import { AnchoredMicroTheoryBlock } from "./AnchoredMicroTheoryBlock";
import { BranchingScenarioChallengeBlock } from "./BranchingScenarioChallengeBlock";
import { DeclarativeVisualDiagramBlock } from "./DeclarativeVisualDiagramBlock";
import { HandsOnMissionBlock } from "./HandsOnMissionBlock";
import { HeuristicErrorAuditBlock } from "./HeuristicErrorAuditBlock";
import { InteractivePredictionGateBlock } from "./InteractivePredictionGateBlock";
import { MetacognitiveClosureBlock } from "./MetacognitiveClosureBlock";
import { BlockLexicalAssistant } from "../LexicalAssistantPopover";

const BLOCK_LABEL: Record<string, string> = {
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

const TERM_MAX_CHARS = 100;

/** What the popover assistant needs to answer well FOR THIS SPECIFIC BLOCK —
 *  never the whole lesson's content, just this block's own. `answerBearing`
 *  is a defensive redaction guard only (see `LexicalAssistantService::ask`),
 *  never sent into the assistant's own prompt. */
function deriveBlockLexicalContext(content: DynamicSectionBlock): { term: string; fragmentContext: string; answerBearing: string[] } {
  switch (content.blockType) {
    case "anchored_micro_theory":
      return { term: content.title, fragmentContext: `${content.intuitiveHook} ${content.systemRule}`, answerBearing: [] };
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
      return { term: content.question, fragmentContext: content.options.join(", "), answerBearing: content.options };
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
      </section>
    );
  }
  const content = parsed.data;
  const statusLabel = block.status !== "ready" ? STATUS_LABEL[block.status] : undefined;
  const lexical = deriveBlockLexicalContext(content);

  return (
    <section className={`notebook-block-wrapper ${content.blockType} status-${block.status}`}>
      <div className="block-kicker">
        <span className="block-kicker-label">
          {BLOCK_LABEL[content.blockType] ?? content.blockType}
          {statusLabel && <span className={`badge ${block.status === "passed" ? "ok" : "warn"}`}>{statusLabel}</span>}
        </span>
        <BlockLexicalAssistant
          term={lexical.term.slice(0, TERM_MAX_CHARS)}
          fragmentContext={lexical.fragmentContext || null}
          keyConcepts={lessonKeyConcepts}
          answerBearingStrings={lexical.answerBearing}
        />
      </div>
      {content.blockType === "anchored_micro_theory" && <AnchoredMicroTheoryBlock content={content} />}
      {content.blockType === "declarative_visual_diagram" && <DeclarativeVisualDiagramBlock content={content} />}
      {content.blockType === "branching_scenario_challenge" && (
        <BranchingScenarioChallengeBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} />
      )}
      {content.blockType === "heuristic_error_audit" && (
        <HeuristicErrorAuditBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} />
      )}
      {content.blockType === "interactive_prediction_gate" && (
        <InteractivePredictionGateBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} />
      )}
      {content.blockType === "hands_on_mission" && (
        <HandsOnMissionBlock block={block} content={content} isActive={isActive} onSubmitGate={onSubmitGate} />
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
    </section>
  );
}
