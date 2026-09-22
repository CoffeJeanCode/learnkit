import { create } from "zustand";
import type { RoadmapSession } from "../lib/schemas";
import { getRoadmapSession, sendRoadmapMessage, startRoadmapSession, tauriError } from "../lib/tauri";
import type { ChatMessage } from "../types";

interface RoadmapState {
  session: RoadmapSession | null;
  messages: ChatMessage[];
  sending: boolean;
  error: string | null;
  start: () => Promise<void>;
  send: (input: string) => Promise<void>;
  refreshSession: () => Promise<void>;
  reset: () => void;
}

export const useRoadmap = create<RoadmapState>((set, get) => ({
  session: null,
  messages: [],
  sending: false,
  error: null,

  start: async () => {
    set({ sending: true, error: null, messages: [], session: null });
    try {
      const { message, session } = await startRoadmapSession();
      set({ session, messages: [{ role: "assistant", content: message }], sending: false });
    } catch (e) {
      const err = tauriError(e);
      set({ sending: false, error: `${err.code}: ${err.message}` });
    }
  },

  send: async (input) => {
    const { session } = get();
    if (!session || !input.trim() || session.status === "sealed") return;
    set((s) => ({
      sending: true,
      error: null,
      messages: [...s.messages, { role: "user", content: input }],
    }));
    try {
      const { message, session: updated } = await sendRoadmapMessage(session.session_id, input);
      set((s) => ({
        sending: false,
        session: updated,
        messages: [...s.messages, { role: "assistant", content: message }],
      }));
    } catch (e) {
      const err = tauriError(e);
      set((s) => ({
        sending: false,
        error: `${err.code}: ${err.message}`,
        messages: [...s.messages, { role: "error", content: `${err.code}: ${err.message}` }],
      }));
    }
  },

  refreshSession: async () => {
    const { session } = get();
    if (!session) return;
    try {
      const fresh = await getRoadmapSession(session.session_id);
      set({ session: fresh });
    } catch (e) {
      set({ error: tauriError(e).message });
    }
  },

  reset: () => set({ session: null, messages: [], sending: false, error: null }),
}));
