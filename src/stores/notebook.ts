import { create } from "zustand";
import type { ClassRecord } from "../lib/schemas";

// Last class the student was reading — kept in localStorage so "Clases"
// resumes where they left off even across an app restart (the in-memory
// store alone dies with the window).
const ACTIVE_CLASS_KEY = "learnkit-active-class";

function readActiveClass(): string | null {
  try {
    return window.localStorage.getItem(ACTIVE_CLASS_KEY);
  } catch {
    return null;
  }
}

function writeActiveClass(id: string | null): void {
  try {
    if (id) window.localStorage.setItem(ACTIVE_CLASS_KEY, id);
    else window.localStorage.removeItem(ACTIVE_CLASS_KEY);
  } catch {
    // Storage unavailable (private mode) — resume falls back to the first class.
  }
}

interface NotebookNavState {
  classes: ClassRecord[];
  courseId: string | null;
  activeClassId: string | null;
  /** Opens the notebook view for `classId`, remembering the sibling classes
   *  of its course so the view can offer a week switcher. */
  open: (classes: ClassRecord[], classId: string, courseId: string) => void;
  setActiveClass: (classId: string) => void;
  /** Re-syncs the course's classes after a change that can flip a class to
   *  `complete` (gate resolved, reflection written) — the path's locks and
   *  checks re-derive from that flag. */
  setClasses: (classes: ClassRecord[]) => void;
}

export const useNotebookNav = create<NotebookNavState>((set) => ({
  classes: [],
  courseId: null,
  activeClassId: readActiveClass(),
  open: (classes, classId, courseId) => {
    writeActiveClass(classId);
    set({ classes, activeClassId: classId, courseId });
  },
  setActiveClass: (classId) => {
    writeActiveClass(classId);
    set({ activeClassId: classId });
  },
  setClasses: (classes) => set({ classes }),
}));
