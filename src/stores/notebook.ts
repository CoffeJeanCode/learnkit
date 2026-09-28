import { create } from "zustand";
import type { ClassRecord } from "../lib/schemas";
import { listCourseClasses } from "../lib/tauri";

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
  /** Loads every class of `courseId` and resumes the last one read (or the
   *  first when that class is stale/gone) WITHOUT changing the view — the
   *  header's "Clases" tab and the boot restore both go through here, so
   *  neither has to reimplement the resume rule. Throws on failure so the
   *  caller decides what the student sees. */
  openCourse: (courseId: string) => Promise<void>;
  setActiveClass: (classId: string) => void;
  /** Re-syncs the course's classes after a change that can flip a class to
   *  `complete` (gate resolved, reflection written) — the path's locks and
   *  checks re-derive from that flag. */
  setClasses: (classes: ClassRecord[]) => void;
  /** Drops the loaded course entirely — called when the session that owned
   *  it is deleted, so a tab can't keep pointing at a course the cascade
   *  already removed from SQLite. */
  clear: () => void;
}

export const useNotebookNav = create<NotebookNavState>((set, get) => ({
  classes: [],
  courseId: null,
  activeClassId: readActiveClass(),
  open: (classes, classId, courseId) => {
    writeActiveClass(classId);
    set({ classes, activeClassId: classId, courseId });
  },
  openCourse: async (courseId) => {
    const classes = await listCourseClasses(courseId);
    // Resume the last class the student was reading, falling back to the
    // first when it's stale — the class may belong to another course.
    const restored = get().activeClassId;
    const target = classes.some((c) => c.id === restored) ? restored! : (classes[0]?.id ?? null);
    if (target) writeActiveClass(target);
    set({ classes, courseId, activeClassId: target });
  },
  setActiveClass: (classId) => {
    writeActiveClass(classId);
    set({ activeClassId: classId });
  },
  setClasses: (classes) => set({ classes }),
  clear: () => {
    writeActiveClass(null);
    set({ classes: [], courseId: null, activeClassId: null });
  },
}));
