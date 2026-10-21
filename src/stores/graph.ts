import { create } from "zustand";
import {
  deleteConcept,
  deleteConceptLink,
  getConceptGraph,
  listCourseClasses,
  saveConcept,
  saveConceptLink,
  tauriError,
} from "../lib/tauri";
import type { ClassRecord, Concept, ConceptGraph } from "../lib/schemas";

// The student's knowledge graph for ONE course. Everything here is written by
// the student; nothing asks a model for anything. Mutations call the backend
// (which validates) and then reload, so the screen always shows what is stored.

interface GraphState {
  courseId: string | null;
  classes: ClassRecord[];
  graph: ConceptGraph;
  loading: boolean;
  error: string | null;
  load: (courseId: string) => Promise<void>;
  clearError: () => void;
  /** The saved concept, or `null` when the backend rejected it (see `error`). */
  addConcept: (classId: string, label: string) => Promise<Concept | null>;
  renameConcept: (id: string, classId: string, label: string) => Promise<boolean>;
  removeConcept: (id: string) => Promise<boolean>;
  addLink: (fromId: string, toId: string, label: string) => Promise<boolean>;
  editLink: (id: string, fromId: string, toId: string, label: string) => Promise<boolean>;
  removeLink: (id: string) => Promise<boolean>;
}

const EMPTY: ConceptGraph = { concepts: [], links: [] };

export const useGraph = create<GraphState>((set, get) => {
  // Runs a backend write, then reloads the graph. Returns what the write
  // returned, or `null` when it failed — the validation message (Spanish, from
  // the backend) lands in `error`.
  const mutate = async <T>(write: (courseId: string) => Promise<T>): Promise<T | null> => {
    const courseId = get().courseId;
    if (!courseId) return null;
    set({ error: null });
    try {
      const result = await write(courseId);
      set({ graph: await getConceptGraph(courseId) });
      return result;
    } catch (e) {
      set({ error: tauriError(e).message });
      return null;
    }
  };
  const ok = async (write: (courseId: string) => Promise<unknown>): Promise<boolean> => (await mutate(async (c) => (await write(c), true))) === true;

  return {
    courseId: null,
    classes: [],
    graph: EMPTY,
    loading: false,
    error: null,
    load: async (courseId) => {
      set({ courseId, loading: true, error: null });
      try {
        const [classes, graph] = await Promise.all([listCourseClasses(courseId), getConceptGraph(courseId)]);
        set({ classes, graph, loading: false });
      } catch (e) {
        set({ error: tauriError(e).message, loading: false });
      }
    },
    clearError: () => set({ error: null }),
    addConcept: (classId, label) => mutate((c) => saveConcept(c, classId, label)),
    renameConcept: (id, classId, label) => ok((c) => saveConcept(c, classId, label, id)),
    removeConcept: (id) => ok((c) => deleteConcept(c, id)),
    addLink: (fromId, toId, label) => ok((c) => saveConceptLink(c, fromId, toId, label)),
    editLink: (id, fromId, toId, label) => ok((c) => saveConceptLink(c, fromId, toId, label, id)),
    removeLink: (id) => ok((c) => deleteConceptLink(c, id)),
  };
});
