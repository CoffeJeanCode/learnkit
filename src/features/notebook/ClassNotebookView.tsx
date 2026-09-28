import { useEffect, useState } from "react";
import { DynamicSectionBlockSchema } from "../../lib/schemas";
import type { DynamicBlockType, NotebookBlock } from "../../lib/schemas";
import {
  gradeClosureReflection,
  listCourseClasses,
  saveNotebookState,
  startClassNotebook,
  tauriError,
} from "../../lib/tauri";
import { useNotebookNav } from "../../stores/notebook";
import { useRoadmap } from "../../stores/roadmap";
import { useUi } from "../../stores/ui";
import { ClassGenerationLoader, ThinkingSketch } from "../../components/AgentLoader";
import { ClassPath } from "../../components/ClassPath";
import { DynamicNotebookBlock } from "./blocks";
import { useClassNotebookSession } from "./useClassNotebookSession";

// One-class-ahead buffer: while the student reads class N, class N+1's
// first block starts generating silently in the background, so by the time
// they navigate there it's already waiting instead of showing the starting
// loader. Deduped by a module-level set — best-effort only, a failure here
// is invisible; the student's own visit falls back to the normal path.
const notebookPrefetched = new Set<string>();

function prefetchClassNotebook(classId: string) {
  if (notebookPrefetched.has(classId)) return;
  notebookPrefetched.add(classId);
  startClassNotebook(classId).catch(() => notebookPrefetched.delete(classId));
}

const GATE_BLOCK_TYPES: readonly DynamicBlockType[] = [
  "interactive_prediction_gate",
  "branching_scenario_challenge",
  "heuristic_error_audit",
  "hands_on_mission",
];

function isUnresolvedGate(block: NotebookBlock): boolean {
  return GATE_BLOCK_TYPES.includes(block.block_type) && block.status !== "passed" && block.status !== "escalated";
}

/** Short labels for the popover assistant's `keyConcepts` — derived from
 *  whatever's already visible on screen, never fetched separately. */
function deriveKeyConcepts(blocks: NotebookBlock[]): string[] {
  const labels = new Set<string>();
  for (const block of blocks) {
    const parsed = DynamicSectionBlockSchema.safeParse(block.content_json);
    if (!parsed.success) continue;
    const c = parsed.data;
    const label =
      c.blockType === "anchored_micro_theory" || c.blockType === "declarative_visual_diagram"
        ? c.title
        : c.blockType === "branching_scenario_challenge"
          ? c.scenario
          : c.blockType === "interactive_prediction_gate"
            ? c.question
            : c.blockType === "heuristic_error_audit"
              ? c.instruction
              : c.blockType === "hands_on_mission"
                ? c.challengeStatement
                : c.blockType === "spaced_interleaved_retrieval"
                  ? c.items[0]?.conceptLabel
                  : c.synthesisTask;
    if (label) labels.add(label.slice(0, 80));
  }
  return [...labels].slice(0, 8);
}

/** The "Clases" section: the class notebook itself — a mastery-gated
 *  sequence of pedagogical blocks, one visible at a time, the next buffered
 *  in the background while the student reads/works the current one; no block
 *  past an unresolved gate is ever shown.
 *
 *  The plan is NOT a tab in here anymore — it's its own section in the
 *  header (see `SessionPlanView`). The sidebar keeps a compact plan note plus
 *  the Duolingo-style class path, so the plan stays visible and navigable
 *  without leaving the notebook. */
export function ClassNotebookView() {
  const { classes, activeClassId, courseId } = useNotebookNav();
  const setActiveClass = useNotebookNav((s) => s.setActiveClass);
  const setClasses = useNotebookNav((s) => s.setClasses);
  const setView = useUi((s) => s.setView);
  const roadmapSession = useRoadmap((s) => s.session);
  const [reloadToken, setReloadToken] = useState(0);
  const [visibleCount, setVisibleCount] = useState(1);

  // --- Sidebar plan note -----------------------------------------------
  // Only when the open session actually OWNS this course: the header's
  // sections belong to the active session, so an unrelated conversation must
  // never lend its plan to a class it didn't create.
  const ownsCourse = roadmapSession !== null && roadmapSession.imported_course_id === courseId;
  const pkg = ownsCourse ? roadmapSession?.roadmap_package?.syllabus ?? null : null;

  const classId = activeClassId;
  const {
    document,
    blocks,
    bufferState,
    bufferError,
    loading,
    error,
    stage,
    submitGate,
    retryPendingBlock,
    regenerateBlock,
    regeneratingBlockId,
  } = useClassNotebookSession(classId, reloadToken);
  const activeClass = classes.find((c) => c.id === classId);
  // Which block the "Regenerar" action failed for, and why — shown on THAT
  // block's card so a failure never hides behind an unrelated one.
  const [regenerateFailure, setRegenerateFailure] = useState<{ blockId: string; message: string } | null>(null);

  // Re-syncs the sibling classes after a change that can flip this class to
  // `complete` (last gate resolved, reflection written) — the sidebar path
  // re-derives its locks/checkmarks from that flag, and the prefetch effect
  // below re-evaluates against it. Best-effort: a failed refresh only means
  // the path shows slightly stale lock state until the next visit.
  const refreshClasses = async () => {
    if (!courseId) return;
    try {
      setClasses(await listCourseClasses(courseId));
    } catch (e) {
      console.warn("no se pudo actualizar la ruta de clases", tauriError(e).message);
    }
  };

  // Reset pacing to the first block on every class switch, and kick off the
  // next class's silent buffer the instant this class is complete (its own
  // gates + the written closure — the same rule the backend guard enforces;
  // a prefetch attempted earlier would just bounce off that guard). Re-runs
  // whenever the refreshed `classes` arrive, which is what re-triggers a
  // prefetch that previously bounced.
  useEffect(() => {
    setVisibleCount(1);
    setRegenerateFailure(null);
  }, [classId]);
  useEffect(() => {
    if (!classId || blocks.length === 0) return;
    const idx = classes.findIndex((c) => c.id === classId);
    if (idx < 0 || !classes[idx].complete) return;
    const next = classes[idx + 1];
    if (next) prefetchClassNotebook(next.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [classId, classes, blocks.length > 0]);

  const visibleBlocks = blocks.slice(0, visibleCount);
  const lastVisible = visibleBlocks[visibleBlocks.length - 1];
  const nextBufferedBlock = blocks[visibleCount];
  const awaitingGate = !!lastVisible && isUnresolvedGate(lastVisible);
  const lessonKeyConcepts = deriveKeyConcepts(visibleBlocks);

  // Auto-advance through content blocks — the student never has to click
  // "Continuar" just to keep reading. The ONLY thing that ever holds the
  // notebook in place is a real activity: an unresolved gate. The instant a
  // buffered block exists AND the current one isn't a gate awaiting a
  // submission, reveal it immediately; this cascades on its own (each reveal
  // re-triggers the effect) until it lands on either a gate or the end of
  // what's been generated so far.
  useEffect(() => {
    if (!awaitingGate && nextBufferedBlock) {
      setVisibleCount((n) => n + 1);
    }
  }, [awaitingGate, nextBufferedBlock]);

  const handleSubmitGate = async (blockId: string, submission: Parameters<typeof submitGate>[1]) => {
    const result = await submitGate(blockId, submission);
    // Passing (or exhausting) a gate IS the "continue" action — content
    // blocks past it auto-reveal on their own (see the effect above).
    if (result.passed || result.escalation_block) {
      setVisibleCount((n) => n + 1);
    }
    // The submission may have flipped this class's last unresolved gate —
    // re-sync the path so the next class's lock state updates immediately.
    void refreshClasses();
    return result;
  };

  // The only remaining non-gating mutation: the closing block's free-text
  // reflection (see `MetacognitiveClosureBlock`) — never touches gating
  // state directly, but a written reflection can be the last piece that
  // makes this class `complete`, so the path re-syncs once it lands.
  const handleSaveReflection = (blockId: string, content: Record<string, unknown>) => {
    if (!document) return;
    saveNotebookState(document.id, [{ id: blockId, content_json: content }])
      .then(() => {
        void refreshClasses();
      })
      .catch((e) => {
        // Best-effort: a failed reflection save is not worth surfacing as a
        // blocking error over the notebook itself.
        console.warn("no se pudo guardar la reflexión", tauriError(e).message);
      });
  };

  // The closing block's graded feedback: the backend persists the exact
  // reflection it grades, so this can land completion even if the debounced
  // `handleSaveReflection` hadn't fired yet. A pass is the LAST piece of
  // this class — re-sync the path so the next class's lock (and its
  // one-class-ahead prefetch) updates immediately.
  const handleRequestClosureFeedback = async (blockId: string, reflection: string) => {
    try {
      const result = await gradeClosureReflection(blockId, reflection);
      void refreshClasses();
      return result;
    } catch (e) {
      throw new Error(tauriError(e).message);
    }
  };

  const handleRegenerateBlock = async (blockId: string) => {
    setRegenerateFailure(null);
    try {
      await regenerateBlock(blockId);
    } catch (e) {
      setRegenerateFailure({ blockId, message: (e as Error).message });
    }
  };

  const classIndex = classes.findIndex((c) => c.id === classId);
  const nextClass = classIndex >= 0 ? classes[classIndex + 1] : undefined;
  const continueLabel = nextClass ? "Continuar con el módulo" : "Volver a tu plan";
  const handleContinueModule = () => {
    if (nextClass) setActiveClass(nextClass.id);
    else setView("plan");
  };

  if (!classId) {
    return (
      <div className="view notebook">
        <p className="muted">Abre un notebook desde tu plan guardado.</p>
      </div>
    );
  }

  return (
    <div className="view notebook">
      <div className="notebook-shell">
        <div className="notebook-main">
          <div className="row notebook-header">
            <h2>{activeClass?.title ?? "Clase"}</h2>
          </div>
          {/* What this class lets you DO when it's done — the micromodule's
              own objective, stamped onto the class at import time. Hidden
              for classes imported before the field existed. */}
          {activeClass?.objective && <p className="hint class-objective">Objetivo: {activeClass.objective}</p>}

          {loading && <ClassGenerationLoader stage={stage} />}
          {error && (
            <div className="alert error">
              <span className="alert-text">{error}</span>
              <button className="btn-quiet btn-retry" onClick={() => setReloadToken((n) => n + 1)} disabled={loading}>
                Reintentar
              </button>
            </div>
          )}

          {!loading && !error && (
            <div className="notebook-blocks">
              {visibleBlocks.map((block) => (
                <DynamicNotebookBlock
                  key={block.id}
                  block={block}
                  isActive={block.id === lastVisible?.id}
                  lessonKeyConcepts={lessonKeyConcepts}
                  onSubmitGate={handleSubmitGate}
                  onSave={handleSaveReflection}
                  onRequestClosureFeedback={handleRequestClosureFeedback}
                  onContinueModule={handleContinueModule}
                  continueLabel={continueLabel}
                  onRegenerate={handleRegenerateBlock}
                  regenerating={regeneratingBlockId === block.id}
                  regenerateError={regenerateFailure?.blockId === block.id ? regenerateFailure.message : null}
                />
              ))}

              {!awaitingGate &&
                !nextBufferedBlock &&
                (bufferState === "error" ? (
                  <div className="alert error">
                    <span className="alert-text">{bufferError ?? "No se pudo generar el siguiente bloque."}</span>
                    <button className="btn-quiet btn-retry" onClick={() => void retryPendingBlock()}>
                      Reintentar
                    </button>
                  </div>
                ) : lastVisible && lastVisible.block_type !== "metacognitive_closure" ? (
                  <ThinkingSketch label="Preparando el siguiente paso…" />
                ) : null)}
            </div>
          )}
        </div>

        {(pkg || classes.length > 0) && (
          <aside className="notebook-sidebar">
            {pkg && (
              <div className="card side-note">
                <div className="card-head">
                  <h3>{pkg.courseTitle}</h3>
                  <span className="badge ok">tu plan</span>
                </div>
                <p className="hint">
                  {pkg.totalWeeks} semanas · {pkg.paceHoursPerWeek} h/semana
                </p>
                <div className="row">
                  <button className="btn-quiet" onClick={() => setView("plan")}>
                    Ver tu plan →
                  </button>
                </div>
              </div>
            )}

            {classes.length > 0 && (
              <div className="card class-path-card">
                <div className="card-head">
                  <h3>Tu ruta</h3>
                </div>
                <ClassPath classes={classes} activeClassId={classId} onSelect={setActiveClass} />
              </div>
            )}
          </aside>
        )}
      </div>
    </div>
  );
}
