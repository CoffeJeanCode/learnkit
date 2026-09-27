import { create } from "zustand";
import type { View } from "../types";

interface UiState {
  view: View;
  /** The last main section visited (conversation or classes) — "← Volver"
   *  from the providers screen returns there instead of always landing on
   *  the roadmap. */
  lastMainView: "roadmap" | "notebook";
  setView: (v: View) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "roadmap",
  lastMainView: "roadmap",
  setView: (view) =>
    set((s) => ({
      view,
      lastMainView: view === "roadmap" || view === "notebook" ? view : s.lastMainView,
    })),
}));
