import { create } from "zustand";
import {
  deleteProviderKey,
  listModels,
  listProviders,
  saveProvider,
  saveProviderKey,
  suggestedModels,
  tauriError,
  testProvider,
} from "../lib/tauri";
import type { ModelInfo, ProviderWithStatus } from "../types";

interface ProvidersState {
  providers: ProviderWithStatus[];
  modelsCache: Record<string, ModelInfo[]>;
  suggestionsCache: Record<string, string[]>;
  loading: boolean;
  busyId: string | null;
  error: string | null;
  notice: string | null;
  refresh: () => Promise<void>;
  saveConfig: (id: string, patch: Partial<ProviderWithStatus>) => Promise<void>;
  saveKey: (id: string, key: string) => Promise<void>;
  removeKey: (id: string) => Promise<void>;
  test: (id: string) => Promise<void>;
  loadModels: (id: string) => Promise<void>;
  loadSuggestions: (provider: string) => Promise<void>;
  clearNotice: () => void;
}

export const useProviders = create<ProvidersState>((set, get) => ({
  providers: [],
  modelsCache: {},
  suggestionsCache: {},
  // Starts true (not false): every consumer calls `refresh()` in a mount
  // effect, which runs AFTER the first paint. Defaulting to false let that
  // first frame render with an empty `providers` array as if loading were
  // already done — e.g. the onboarding provider dropdown briefly (or, on a
  // slow first IPC round-trip, not-so-briefly) showing as empty.
  loading: true,
  busyId: null,
  error: null,
  notice: null,

  refresh: async () => {
    set({ loading: true, error: null });
    try {
      const providers = await listProviders();
      set({ providers, loading: false });
    } catch (e) {
      set({ error: tauriError(e).message, loading: false });
    }
  },

  saveConfig: async (id, patch) => {
    const current = get().providers.find((p) => p.id === id);
    if (!current) return;
    set({ busyId: id, error: null, notice: null });
    try {
      await saveProvider({
        id: current.id,
        provider: current.provider,
        name: patch.name ?? current.name,
        default_model:
          patch.default_model !== undefined ? patch.default_model : current.default_model,
        base_url: patch.base_url !== undefined ? patch.base_url : current.base_url,
      });
      set({ notice: `Provider ${id} saved` });
      await get().refresh();
    } catch (e) {
      set({ error: tauriError(e).message });
    } finally {
      set({ busyId: null });
    }
  },

  saveKey: async (id, key) => {
    set({ busyId: id, error: null, notice: null });
    try {
      await saveProviderKey(id, key);
      set({ notice: `Key stored for ${id} (Configured ✓)` });
      await get().refresh();
    } catch (e) {
      set({ error: tauriError(e).message });
    } finally {
      set({ busyId: null });
    }
  },

  removeKey: async (id) => {
    set({ busyId: id, error: null, notice: null });
    try {
      await deleteProviderKey(id);
      set({ notice: `Key deleted for ${id}` });
      await get().refresh();
    } catch (e) {
      set({ error: tauriError(e).message });
    } finally {
      set({ busyId: null });
    }
  },

  test: async (id) => {
    set({ busyId: id, error: null, notice: null });
    try {
      await testProvider(id);
      set({ notice: `Connection OK for ${id}` });
    } catch (e) {
      const err = tauriError(e);
      set({ error: `${err.code}: ${err.message}` });
    } finally {
      set({ busyId: null });
    }
  },

  loadModels: async (id) => {
    set({ busyId: id, error: null });
    try {
      const models = await listModels(id);
      set((s) => ({ modelsCache: { ...s.modelsCache, [id]: models } }));
    } catch (e) {
      set({ error: tauriError(e).message });
    } finally {
      set({ busyId: null });
    }
  },

  loadSuggestions: async (provider) => {
    if (get().suggestionsCache[provider]) return;
    try {
      const suggestions = await suggestedModels(provider);
      set((s) => ({ suggestionsCache: { ...s.suggestionsCache, [provider]: suggestions } }));
    } catch {
      // Non-critical; manual model ids always allowed.
    }
  },

  clearNotice: () => set({ notice: null, error: null }),
}));
