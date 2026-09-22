import { create } from "zustand";
import { onAgentEvent, runAgent, tauriError } from "../lib/tauri";
import type { ChatMessage } from "../types";

interface ChatState {
  agentId: string | null;
  messages: ChatMessage[];
  running: boolean;
  currentRunId: string | null;
  status: string | null;
  error: string | null;
  listenersReady: boolean;
  selectAgent: (id: string | null) => void;
  send: (input: string) => Promise<void>;
  appendMessage: (m: ChatMessage) => void;
  clear: () => void;
  ensureListeners: () => Promise<void>;
}

export const useChat = create<ChatState>((set, get) => ({
  agentId: null,
  messages: [],
  running: false,
  currentRunId: null,
  status: null,
  error: null,
  listenersReady: false,

  selectAgent: (id) => set({ agentId: id }),

  appendMessage: (m) => set((s) => ({ messages: [...s.messages, m] })),

  clear: () => set({ messages: [], error: null, status: null, currentRunId: null }),

  ensureListeners: async () => {
    if (get().listenersReady) return;
    try {
      await onAgentEvent("agent://started", (e) => {
        set({ currentRunId: e.run_id, status: `Running ${e.agent_id}…` });
        get().appendMessage({ role: "status", content: `▶ started ${e.agent_id}`, run_id: e.run_id });
      });
      await onAgentEvent("agent://completed", (e) => {
        set({ status: null, running: false, currentRunId: null });
        get().appendMessage({
          role: "assistant",
          content: e.text ?? "",
          run_id: e.run_id,
        });
      });
      await onAgentEvent("agent://error", (e) => {
        set({ status: null, running: false, currentRunId: null });
        get().appendMessage({ role: "error", content: e.error ?? "unknown error", run_id: e.run_id });
      });
      set({ listenersReady: true });
    } catch {
      // Running outside Tauri (plain `bun run dev`): events unavailable, still usable via invoke fallback.
      set({ listenersReady: true });
    }
  },

  send: async (input) => {
    const { agentId } = get();
    if (!agentId || !input.trim()) return;
    set({ running: true, error: null, status: `Running ${agentId}…` });
    get().appendMessage({ role: "user", content: input });
    try {
      await get().ensureListeners();
      const out = await runAgent(agentId, input);
      // If events are unavailable (browser dev), render the result directly.
      const alreadyRendered = get().messages.some(
        (m) => m.run_id === out.run_id && m.role === "assistant",
      );
      if (!alreadyRendered) {
        set({ status: null, running: false });
        get().appendMessage({ role: "assistant", content: out.text, run_id: out.run_id });
      }
    } catch (e) {
      const err = tauriError(e);
      set({ running: false, status: null, error: `${err.code}: ${err.message}` });
      get().appendMessage({ role: "error", content: `${err.code}: ${err.message}` });
    }
  },
}));
