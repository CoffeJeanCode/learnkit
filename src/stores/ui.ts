import { create } from "zustand";
import type { View } from "../types";

interface UiState {
  view: View;
  setView: (v: View) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "roadmap",
  setView: (view) => set({ view }),
}));
