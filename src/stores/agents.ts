import { create } from "zustand";
import {
  createAgent,
  deleteAgent,
  listAgents,
  listTools,
  tauriError,
} from "../lib/tauri";
import type { AgentDefinition, ToolInfo } from "../types";

interface AgentsState {
  agents: AgentDefinition[];
  tools: ToolInfo[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  create: (agent: AgentDefinition) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useAgents = create<AgentsState>((set, get) => ({
  agents: [],
  tools: [],
  loading: false,
  error: null,

  refresh: async () => {
    set({ loading: true, error: null });
    try {
      const [agents, tools] = await Promise.all([listAgents(), listTools()]);
      set({ agents, tools, loading: false });
    } catch (e) {
      set({ error: tauriError(e).message, loading: false });
    }
  },

  create: async (agent) => {
    set({ error: null });
    try {
      await createAgent(agent);
      await get().refresh();
    } catch (e) {
      const err = tauriError(e);
      set({ error: `${err.code}: ${err.message}` });
      throw e;
    }
  },

  remove: async (id) => {
    set({ error: null });
    try {
      await deleteAgent(id);
      await get().refresh();
    } catch (e) {
      set({ error: tauriError(e).message });
    }
  },
}));
