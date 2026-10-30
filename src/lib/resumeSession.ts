import { create } from "zustand";
import { useNotebookNav } from "../stores/notebook";
import { useRoadmap } from "../stores/roadmap";
import { useUi } from "../stores/ui";
import { browserStore, landing, loadResume } from "./resume";

/** The block to scroll to once a notebook has loaded. `ClassNotebookView` consumes it. */
export const useResumeTarget = create<{ pending: { classId: string; blockId: string } | null }>(() => ({ pending: null }));

let restoring = false;
/** True while a session is being opened: the position writers stay quiet so they
 *  never overwrite the saved position with the screen the app happens to start on. */
export const isRestoring = () => restoring;

/** Queues the saved block of `sessionId` when it belongs to the class now active. */
export function queueBlockResume(sessionId: string): void {
  const resume = loadResume(browserStore(), sessionId);
  const classId = useNotebookNav.getState().activeClassId;
  if (resume?.blockId && classId && resume.blockClassId === classId) {
    useResumeTarget.setState({ pending: { classId, blockId: resume.blockId } });
  }
}

/** Opens a saved session and lands where the student left it: the class and the
 *  block inside the notebook, or the plan / conversation. Anything that fails
 *  falls back to the plan, which is always safe. */
export async function resumeSession(id: string): Promise<void> {
  restoring = true;
  try {
    await useRoadmap.getState().openSession(id);
    const session = useRoadmap.getState().session;
    const dest = landing(session, loadResume(browserStore(), id));
    if (dest.view === "notebook" && dest.courseId && dest.classId) {
      try {
        const nav = useNotebookNav.getState();
        // `openCourse` resumes the active class when it belongs to the course, else the first.
        nav.setActiveClass(dest.classId);
        await nav.openCourse(dest.courseId);
        useResumeTarget.setState({ pending: null });
        if (dest.blockId && useNotebookNav.getState().activeClassId === dest.classId) {
          useResumeTarget.setState({ pending: { classId: dest.classId, blockId: dest.blockId } });
        }
        useUi.getState().setView("notebook");
        return;
      } catch {
        // The course could not be loaded: the plan still opens.
      }
    }
    useUi.getState().setView(dest.view === "notebook" ? "plan" : dest.view);
  } finally {
    restoring = false;
  }
}
