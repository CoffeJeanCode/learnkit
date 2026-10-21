import { create } from "zustand";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { appendEntry, type DevTraceEntry } from "../lib/devTrace";
import { clearDevTrace, getDevMode, getDevTrace, onDevTrace, setDevMode } from "../lib/tauri";

interface DevState {
  enabled: boolean;
  panelOpen: boolean;
  entries: DevTraceEntry[];
  /** Loads the persisted switch + what was captured, then subscribes (once). */
  init: () => Promise<void>;
  setEnabled: (on: boolean) => Promise<void>;
  setPanelOpen: (open: boolean) => void;
  clear: () => Promise<void>;
}

let unlisten: UnlistenFn | null = null;
let initialized = false;

export const useDev = create<DevState>((set, get) => ({
  enabled: false,
  panelOpen: false,
  entries: [],
  init: async () => {
    if (initialized) return;
    initialized = true;
    try {
      const [enabled, entries] = await Promise.all([getDevMode(), getDevTrace()]);
      set({ enabled, entries });
      unlisten = await onDevTrace((e) => set((s) => ({ entries: appendEntry(s.entries, e) })));
    } catch {
      // Dev mode is optional tooling: never block the app on it.
      initialized = false;
    }
  },
  setEnabled: async (on) => {
    await setDevMode(on);
    set({ enabled: on, panelOpen: on ? get().panelOpen : false, entries: on ? get().entries : [] });
  },
  setPanelOpen: (panelOpen) => set({ panelOpen }),
  clear: async () => {
    await clearDevTrace();
    set({ entries: [] });
  },
}));

export function disposeDevListener() {
  unlisten?.();
  unlisten = null;
  initialized = false;
}
