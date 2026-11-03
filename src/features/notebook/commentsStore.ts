import { create } from "zustand";
import { deleteNotebookComment, listNotebookComments, listCourseComments, upsertNotebookComment } from "../../lib/tauri";
import type { NotebookComment } from "../../lib/schemas";

interface CommentsState {
  notebookId: string | null;
  classId: string | null;
  courseId: string | null;
  comments: NotebookComment[];
  activeCommentId: string | null;
  filter: "all" | "note" | "doubt";
  searchQuery: string;
  isSidebarOpen: boolean;
  isCourseNotesOpen: boolean;

  // Actions
  setScope: (notebookId: string, classId: string, courseId: string) => void;
  setFilter: (filter: "all" | "note" | "doubt") => void;
  setSearchQuery: (query: string) => void;
  setActiveCommentId: (id: string | null) => void;
  setSidebarOpen: (open: boolean) => void;
  setCourseNotesOpen: (open: boolean) => void;
  toggleSidebar: () => void;

  loadComments: (notebookId: string) => Promise<void>;
  saveComment: (comment: NotebookComment) => Promise<void>;
  deleteComment: (commentId: string) => Promise<void>;
  fetchCourseComments: (courseId: string) => Promise<NotebookComment[]>;
}

export const useCommentsStore = create<CommentsState>((set, get) => ({
  notebookId: null,
  classId: null,
  courseId: null,
  comments: [],
  activeCommentId: null,
  filter: "all",
  searchQuery: "",
  isSidebarOpen: false,
  isCourseNotesOpen: false,

  setScope: (notebookId: string, classId: string, courseId: string) => {
    set({ notebookId, classId, courseId });
    void get().loadComments(notebookId);
  },

  setFilter: (filter) => set({ filter }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setActiveCommentId: (activeCommentId) => set({ activeCommentId }),
  setSidebarOpen: (isSidebarOpen) => set({ isSidebarOpen }),
  setCourseNotesOpen: (isCourseNotesOpen) => set({ isCourseNotesOpen }),
  toggleSidebar: () => set((state) => ({ isSidebarOpen: !state.isSidebarOpen })),

  loadComments: async (notebookId: string) => {
    try {
      const comments = await listNotebookComments(notebookId);
      set({ notebookId, comments });
    } catch (e) {
      console.error("Failed to load notebook comments:", e);
    }
  },

  saveComment: async (comment: NotebookComment) => {
    try {
      const saved = await upsertNotebookComment(comment);
      set((state) => {
        const existingIdx = state.comments.findIndex((c) => c.id === saved.id);
        const updated = existingIdx >= 0
          ? state.comments.map((c, i) => (i === existingIdx ? saved : c))
          : [...state.comments, saved];
        return { comments: updated, isSidebarOpen: true };
      });
    } catch (e) {
      console.error("Failed to save comment:", e);
    }
  },

  deleteComment: async (commentId: string) => {
    try {
      await deleteNotebookComment(commentId);
      set((state) => ({
        comments: state.comments.filter((c) => c.id !== commentId),
        activeCommentId: state.activeCommentId === commentId ? null : state.activeCommentId,
      }));
    } catch (e) {
      console.error("Failed to delete comment:", e);
    }
  },

  fetchCourseComments: async (courseId: string) => {
    try {
      return await listCourseComments(courseId);
    } catch (e) {
      console.error("Failed to fetch course comments:", e);
      return [];
    }
  },
}));
