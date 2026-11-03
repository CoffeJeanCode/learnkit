import { create } from "zustand";
import type { NotebookBlock } from "../../lib/schemas";

export interface TokenBreakdown {
  input: number;
  cached: number;
  reasoning: number;
  output: number;
}

export type ToolResolutionMethod = "native" | "salvaged" | "backstop" | "error";

export interface BlockTelemetry {
  blockIndex: number;
  blockType?: string;
  startTime: number;
  ttftMs: number | null;
  totalDurationMs: number | null;
  tokens: TokenBreakdown;
  attempts: number;
  toolResolution: ToolResolutionMethod;
  criticVerdict: "accepted" | "rejected" | "deferred" | null;
  errorMessage?: string;
}

export interface SessionMetrics {
  sessionId: string;
  classId: string;
  documentId: string | null;
  provider: string;
  model: string;
  startedAt: number;
  blocks: BlockTelemetry[];
}

interface SessionObserverState {
  currentSession: SessionMetrics | null;
  pastSessions: SessionMetrics[];
  modalOpen: boolean;

  // Actions
  initSession: (classId: string, provider: string, model: string) => void;
  attachDocumentId: (documentId: string) => void;
  recordGenerating: (blockIndex: number, attempt?: number) => void;
  recordFirstDelta: () => void;
  recordTokenUsage: (usage: Partial<TokenBreakdown>) => void;
  recordReady: (block: NotebookBlock) => void;
  recordError: (message: string) => void;
  recordCriticVerdict: (blockIndex: number, verdict: "accepted" | "rejected" | "deferred") => void;
  setModalOpen: (open: boolean) => void;
  exportMarkdown: () => string;
  clearHistory: () => void;
}

export const useSessionObserver = create<SessionObserverState>((set, get) => ({
  currentSession: null,
  pastSessions: [],
  modalOpen: false,

  initSession: (classId, provider, model) => {
    const prev = get().currentSession;
    const past = prev && prev.blocks.length > 0 ? [prev, ...get().pastSessions.slice(0, 9)] : get().pastSessions;

    set({
      pastSessions: past,
      currentSession: {
        sessionId: `session_${Date.now()}`,
        classId,
        documentId: null,
        provider,
        model,
        startedAt: Date.now(),
        blocks: [],
      },
    });
  },

  attachDocumentId: (documentId) => {
    const cur = get().currentSession;
    if (cur && !cur.documentId) {
      set({ currentSession: { ...cur, documentId } });
    }
  },

  recordGenerating: (blockIndex, attempt = 1) => {
    const cur = get().currentSession;
    if (!cur) return;

    const existingIdx = cur.blocks.findIndex((b) => b.blockIndex === blockIndex);
    const now = Date.now();

    if (existingIdx >= 0) {
      // Reintento o regeneración del mismo bloque
      const updated = [...cur.blocks];
      const existing = updated[existingIdx];
      updated[existingIdx] = {
        ...existing,
        attempts: Math.max(existing.attempts, attempt),
        startTime: now,
        ttftMs: null, // reinicia para medir el TTFT del nuevo intento
      };
      set({ currentSession: { ...cur, blocks: updated } });
    } else {
      // Nuevo bloque
      const newBlock: BlockTelemetry = {
        blockIndex,
        startTime: now,
        ttftMs: null,
        totalDurationMs: null,
        tokens: { input: 0, cached: 0, reasoning: 0, output: 0 },
        attempts: attempt,
        toolResolution: "native",
        criticVerdict: null,
      };
      set({ currentSession: { ...cur, blocks: [...cur.blocks, newBlock] } });
    }
  },

  recordFirstDelta: () => {
    const cur = get().currentSession;
    if (!cur || cur.blocks.length === 0) return;

    const lastIdx = cur.blocks.length - 1;
    const block = cur.blocks[lastIdx];
    if (block.ttftMs === null) {
      const ttft = Math.max(0, Date.now() - block.startTime);
      const updated = [...cur.blocks];
      updated[lastIdx] = { ...block, ttftMs: ttft };
      set({ currentSession: { ...cur, blocks: updated } });
    }
  },

  recordTokenUsage: (usage) => {
    const cur = get().currentSession;
    if (!cur || cur.blocks.length === 0) return;

    const lastIdx = cur.blocks.length - 1;
    const block = cur.blocks[lastIdx];
    const updated = [...cur.blocks];
    updated[lastIdx] = {
      ...block,
      tokens: {
        input: block.tokens.input + (usage.input ?? 0),
        cached: block.tokens.cached + (usage.cached ?? 0),
        reasoning: block.tokens.reasoning + (usage.reasoning ?? 0),
        output: block.tokens.output + (usage.output ?? 0),
      },
    };
    set({ currentSession: { ...cur, blocks: updated } });
  },

  recordReady: (block) => {
    const cur = get().currentSession;
    if (!cur) return;

    const idx = cur.blocks.findIndex((b) => b.blockIndex === block.order_index);
    const now = Date.now();

    if (idx >= 0) {
      const existing = cur.blocks[idx];
      const duration = Math.max(0, now - existing.startTime);
      const updated = [...cur.blocks];
      updated[idx] = {
        ...existing,
        blockType: block.block_type,
        totalDurationMs: duration,
        // Si no registró delta previo (ej. respuesta rápida no-streamed), TTFT es igual a la duración
        ttftMs: existing.ttftMs ?? duration,
      };
      set({ currentSession: { ...cur, blocks: updated } });
    } else {
      // Bloque listo sin evento generating previo
      const newBlock: BlockTelemetry = {
        blockIndex: block.order_index,
        blockType: block.block_type,
        startTime: now,
        ttftMs: 0,
        totalDurationMs: 0,
        tokens: { input: 0, cached: 0, reasoning: 0, output: 0 },
        attempts: 1,
        toolResolution: "native",
        criticVerdict: null,
      };
      set({ currentSession: { ...cur, blocks: [...cur.blocks, newBlock] } });
    }
  },

  recordError: (message) => {
    const cur = get().currentSession;
    if (!cur || cur.blocks.length === 0) return;

    const lastIdx = cur.blocks.length - 1;
    const block = cur.blocks[lastIdx];
    const updated = [...cur.blocks];
    updated[lastIdx] = {
      ...block,
      toolResolution: "error",
      errorMessage: message,
      totalDurationMs: Date.now() - block.startTime,
    };
    set({ currentSession: { ...cur, blocks: updated } });
  },

  recordCriticVerdict: (blockIndex, verdict) => {
    const cur = get().currentSession;
    if (!cur) return;

    const idx = cur.blocks.findIndex((b) => b.blockIndex === blockIndex);
    if (idx >= 0) {
      const updated = [...cur.blocks];
      updated[idx] = { ...updated[idx], criticVerdict: verdict };
      set({ currentSession: { ...cur, blocks: updated } });
    }
  },

  setModalOpen: (open) => set({ modalOpen: open }),

  exportMarkdown: () => {
    const cur = get().currentSession;
    if (!cur) return "No hay sesión activa para reportar.";

    const completed = cur.blocks.filter((b) => b.totalDurationMs !== null);
    const avgDuration = completed.length > 0
      ? (completed.reduce((acc, b) => acc + (b.totalDurationMs ?? 0), 0) / completed.length / 1000).toFixed(2)
      : "N/A";
    const avgTtft = completed.length > 0
      ? (completed.reduce((acc, b) => acc + (b.ttftMs ?? 0), 0) / completed.length / 1000).toFixed(2)
      : "N/A";

    let md = `# Reporte de Sesión de Creación de Notebook\n\n`;
    md += `- **Clase:** \`${cur.classId}\`\n`;
    md += `- **Proveedor:** \`${cur.provider}\`\n`;
    md += `- **Modelo:** \`${cur.model}\`\n`;
    md += `- **Fecha:** ${new Date(cur.startedAt).toLocaleString()}\n`;
    md += `- **Total de bloques generados:** ${completed.length}\n`;
    md += `- **TTFT promedio:** ${avgTtft}s\n`;
    md += `- **Duración promedio por bloque:** ${avgDuration}s\n\n`;

    md += `## Detalle por Bloque\n\n`;
    md += `| # | Tipo de Bloque | Reintentos | TTFT (s) | Duración (s) | In Toks | Cache Toks | Reasoning | Out Toks | Resolución | Crítico |\n`;
    md += `|---|---|---|---|---|---|---|---|---|---|---|\n`;

    for (const b of cur.blocks) {
      const ttft = b.ttftMs !== null ? (b.ttftMs / 1000).toFixed(2) : "-";
      const dur = b.totalDurationMs !== null ? (b.totalDurationMs / 1000).toFixed(2) : "-";
      const type = b.blockType ?? "generando...";
      const critic = b.criticVerdict ?? "-";
      md += `| ${b.blockIndex} | \`${type}\` | ${b.attempts} | ${ttft}s | ${dur}s | ${b.tokens.input} | ${b.tokens.cached} | ${b.tokens.reasoning} | ${b.tokens.output} | \`${b.toolResolution}\` | ${critic} |\n`;
    }

    return md;
  },

  clearHistory: () => set({ pastSessions: [] }),
}));
