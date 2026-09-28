import { create } from "zustand";
import type { View } from "../types";

/** The three sections the header tabs switch between — the parts of the
 *  active session (see `Header`). `providers`/`chat`/etc. are overlays that
 *  keep whatever was last active behind them. */
type MainView = "roadmap" | "plan" | "notebook";

interface UiState {
  view: View;
  /** The last main section visited (conversation, plan or classes) — "←
   *  Volver" from the providers screen returns there instead of always
   *  landing on the roadmap. */
  lastMainView: MainView;
  setView: (v: View) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "roadmap",
  lastMainView: "roadmap",
  setView: (view) =>
    set((s) => ({
      view,
      lastMainView: view === "roadmap" || view === "plan" || view === "notebook" ? view : s.lastMainView,
    })),
}));
