import { create } from "zustand";
import { readActiveClass, useNotebookNav } from "../stores/notebook";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { browserStore, landing, loadResume } from "./resume";

/** The block to scroll to once a notebook has loaded. `ClassNotebookView` consumes it. */
export const useResumeTarget = create<{ pending: { classId: string; blockId: string } | null }>(() => ({ pending: null }));

let restoring = false;
let restoreTimeoutId: ReturnType<typeof setTimeout> | null = null;

/** True while a session is being opened: the position writers stay quiet so they
 *  never overwrite the saved position with the screen the app happens to start on. */
export const isRestoring = () => restoring;

export function finishRestoring(): void {
  if (restoreTimeoutId) {
    clearTimeout(restoreTimeoutId);
    restoreTimeoutId = null;
  }
  restoring = false;
}

/** Queues the saved block of `sessionId` when it belongs to the class now active. */
export function queueBlockResume(sessionId: string): void {
  const resume = loadResume(browserStore(), sessionId);
  const classId = useNotebookNav.getState().activeClassId ?? readActiveClass();
  if (resume?.blockId && classId && resume.blockClassId === classId) {
    useResumeTarget.setState({ pending: { classId, blockId: resume.blockId } });
  }
}

/** Opens a saved session and lands where the student left it: the class and the
 *  block inside the notebook, or the plan / conversation. Anything that fails
 *  falls back to the plan, which is always safe. */
export async function resumeSession(id: string): Promise<void> {
  restoring = true;
  if (restoreTimeoutId) clearTimeout(restoreTimeoutId);

  try {
    await useRoadmap.getState().openSession(id);
    const session = useRoadmap.getState().session;
    const savedResume = loadResume(browserStore(), id);
    let dest = landing(session, savedResume);

    // When the session is sealed and owns a course, ALWAYS resume into the notebook
    // at the last class and block where the student was learning, so entering or restarting
    // the app continues learning directly instead of bouncing to the roadmap.
    if (session?.status === "sealed" && session.imported_course_id) {
      const classId =
        savedResume?.classId ??
        savedResume?.blockClassId ??
        dest.classId ??
        useNotebookNav.getState().activeClassId ??
        readActiveClass() ??
        session.first_class_id ??
        undefined;
      const blockId =
        savedResume?.blockId ??
        dest.blockId ??
        (classId && savedResume?.blockClassId === classId ? savedResume.blockId : undefined);

      if (classId) {
        dest = {
          view: "notebook",
          courseId: session.imported_course_id,
          classId,
          blockId,
        };
      }
    }

    if (dest.view === "notebook" && dest.courseId && dest.classId) {
      try {
        const nav = useNotebookNav.getState();
        nav.setActiveClass(dest.classId);
        await nav.openCourse(dest.courseId, dest.classId);
        useResumeTarget.setState({ pending: null });
        const targetBlock = dest.blockId ?? (savedResume?.blockClassId === dest.classId ? savedResume.blockId : undefined);
        if (targetBlock) {
          useResumeTarget.setState({ pending: { classId: dest.classId, blockId: targetBlock } });
        }
        useUi.getState().setView("notebook");

        // Keep restoring=true for a grace period or until target is scrolled to
        restoreTimeoutId = setTimeout(() => {
          finishRestoring();
        }, 3000);
        return;
      } catch {
        // The course could not be loaded: the plan still opens.
      }
    }
    useUi.getState().setView(dest.view === "notebook" ? "plan" : dest.view);
    finishRestoring();
  } catch {
    finishRestoring();
  }
}
