import { create } from "zustand";
import { getStudySettings, setStudyVariantMode, type StudySettings, type StudyVariantMode } from "../lib/tauri";
import { presentationFor, type Presentation } from "../features/study/presentation";

interface StudyState {
  settings: StudySettings | null;
  load: () => Promise<void>;
  setMode: (mode: StudyVariantMode) => Promise<void>;
}

export const useStudy = create<StudyState>((set) => ({
  settings: null,
  load: async () => {
    try {
      set({ settings: await getStudySettings() });
    } catch {
      // Unreadable settings: stay on the plain presentation (see `presentationFor`).
    }
  },
  setMode: async (mode) => set({ settings: await setStudyVariantMode(mode) }),
}));

/** What to show for the version in force. */
export function usePresentation(): Presentation {
  return presentationFor(useStudy((s) => s.settings?.variant ?? null));
}
