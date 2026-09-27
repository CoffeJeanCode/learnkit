import { create } from "zustand";
import type { ClassRecord } from "../lib/schemas";

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
  activeClassId: null,
  open: (classes, classId, courseId) => set({ classes, activeClassId: classId, courseId }),
  setActiveClass: (classId) => set({ activeClassId: classId }),
  setClasses: (classes) => set({ classes }),
}));
